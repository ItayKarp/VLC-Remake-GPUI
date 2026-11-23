//! Where video plays (S5.4.1): in this window, or, with Settings → Interface "Play video inside
//! main window" off, in a window of its own ([`VideoWindow`]) while this one keeps the controls
//! and shows what plays in its empty state.
//!
//! The video's window opens once the media turns out to have video, and closes a moment after
//! playback stops or the setting is turned back on; the moment lets the queue go from one video
//! to the next without the window closing in between. Closing it stops playback. Fullscreen and
//! "Resize window to fit video" then act on it rather than on this window.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{Context, Subscription, Window, WindowHandle};

use super::MainWindow;
use super::auto_hide::rules::Layout;
use super::now_playing_notice::NowPlayingNotice;
use crate::app::{self, VideoWindowRequest};
use crate::engine::player::Player;
use crate::settings;
use crate::video_window::VideoWindow;
use crate::window_prefs::{Surround, WindowPrefs};

/// How long the video's window stays after it is no longer needed, in case it is again.
const CLOSE_DELAY: Duration = Duration::from_millis(500);

/// Settings → Interface's Window section and notification, for the main window (S5.4).
#[derive(Default)]
pub struct Windowing {
    /// The Window settings as they apply to this window.
    pub(super) prefs: WindowPrefs,
    /// The video's own window.
    pub(super) separate: SeparateVideo,
    /// The video's window is about to close unless it is needed again.
    pub(super) closing: bool,
    /// The item the now-playing notification last named (S5.4.3).
    pub(super) notice: NowPlayingNotice,
    /// Media is opening whose video hasn't shown yet, for "Start videos in fullscreen" (S5.8.1).
    pub(super) start_fullscreen: bool,
    /// The Picture in Picture window (S8.5).
    pub(super) pip: super::pip::Pip,
    /// Whether the first media played pops out into Picture in Picture ("Start in compact
    /// mode").
    pub(super) launch: super::pip::Launch,
}

/// The video's own window.
#[derive(Default)]
pub enum SeparateVideo {
    /// Video plays in the main window.
    #[default]
    Closed,
    /// The window is being opened.
    Opening,
    /// The window is open: whether it is fullscreen, and the subscription noticing it close.
    Open { handle: WindowHandle<VideoWindow>, fullscreen: Rc<Cell<bool>>, _closed: Subscription },
}

impl SeparateVideo {
    /// Whether video plays in a window of its own.
    pub fn is_separate(&self) -> bool {
        !matches!(self, Self::Closed)
    }
}

impl MainWindow {
    /// Opens or closes the video's window to match the setting and what plays.
    pub(super) fn sync_video_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match (self.wants_video_window(cx), &self.windowing.separate) {
            (true, SeparateVideo::Closed) => self.open_video_window(window, cx),
            (false, SeparateVideo::Open { .. }) => self.close_video_window_later(window, cx),
            _ => {}
        }
    }

    /// Whether video should play in a window of its own: the setting is off, and media with
    /// video plays (or keeps playing in the window that is open).
    fn wants_video_window(&self, cx: &Context<Self>) -> bool {
        if settings::get(cx).prefs.interface.play_video_in_main_window
            || self.windowing.pip.is_shown()
        {
            return false;
        }
        let Some((player, _)) = &self.player else { return false };
        let player = player.read(cx);
        player.state().is_active()
            && (player.video_size().is_some() || self.windowing.separate.is_separate())
    }

    fn open_video_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(video), Some((player, _))) = (self.video.clone(), &self.player) else { return };
        let Some(main) = window.window_handle().downcast::<Self>() else { return };
        let Some(size) = player.read(cx).video_size() else { return };
        video.update(cx, |video, _| video.leave(window));
        let fullscreen = Rc::new(Cell::new(false));
        let request = VideoWindowRequest {
            video,
            fullscreen: fullscreen.clone(),
            video_size: (size.width as f32, size.height as f32),
            main,
            display: window.display(cx).map(|display| display.id()),
        };
        self.windowing.separate = SeparateVideo::Opening;
        cx.spawn_in(window, async move |this, cx| {
            let opened = cx.update(|_, cx| app::open_video_window(request, cx));
            this.update_in(cx, |this, window, cx| match opened {
                Ok(Ok(handle)) => this.adopt_video_window(handle, fullscreen, window, cx),
                Ok(Err(err)) | Err(err) => {
                    tracing::error!("couldn't open the video's window: {err:#}");
                    this.video_back_home(window, cx);
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Follows the opened window: the user closing it stops playback.
    fn adopt_video_window(
        &mut self,
        handle: WindowHandle<VideoWindow>,
        fullscreen: Rc<Cell<bool>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Ok(entity) = handle.entity(cx) else { return self.video_back_home(window, cx) };
        let closed = cx.observe_release_in(&entity, window, |this, _, window, cx| {
            this.video_window_closed(window, cx);
        });
        self.windowing.separate = SeparateVideo::Open { handle, fullscreen, _closed: closed };
        // The setting may have changed, or playback stopped, while it opened.
        self.sync_video_window(window, cx);
        self.start_fullscreen(window, cx);
    }

    /// Takes the window showing video fullscreen once its media's picture arrives, as Settings →
    /// Video's "Start videos in fullscreen" says; once per opened media, so leaving fullscreen
    /// sticks. A video window still opening goes fullscreen once it is open.
    pub(super) fn start_fullscreen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.windowing.start_fullscreen
            || matches!(self.windowing.separate, SeparateVideo::Opening)
        {
            return;
        }
        self.windowing.start_fullscreen = false;
        if settings::get(cx).prefs.video.start_fullscreen && !self.video_is_fullscreen(window) {
            self.toggle_fullscreen(window, cx);
        }
    }

    /// The video's window went away. Unless this window closed it, the user did: playback
    /// stops.
    fn video_window_closed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.windowing.separate, SeparateVideo::Open { .. }) {
            return;
        }
        self.video_back_home(window, cx);
        if let Some((player, _)) = &self.player {
            player.update(cx, Player::stop);
        }
    }

    /// Closes the video's window after [`CLOSE_DELAY`] if it still isn't wanted then.
    fn close_video_window_later(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.windowing.closing {
            return;
        }
        self.windowing.closing = true;
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(CLOSE_DELAY).await;
            this.update_in(cx, |this, window, cx| {
                this.windowing.closing = false;
                this.close_unless_wanted(window, cx);
            })
            .ok();
        })
        .detach();
    }

    fn close_unless_wanted(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.wants_video_window(cx) {
            self.close_video_window(window, cx);
        }
    }

    /// Closes the video's window, bringing the video back here.
    pub(super) fn close_video_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let SeparateVideo::Open { handle, .. } = std::mem::take(&mut self.windowing.separate)
        else {
            return;
        };
        cx.defer(move |cx| {
            handle
                .update(cx, |video_window, window, cx| {
                    video_window.release_video(window, cx);
                    window.remove_window();
                })
                .ok();
        });
        self.video_back_home(window, cx);
    }

    /// Video plays in this window again.
    pub(super) fn video_back_home(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.windowing.separate = SeparateVideo::Closed;
        if let Some(video) = &self.video {
            video.update(cx, |video, cx| video.arrive(window, cx));
        }
        cx.notify();
    }

    /// Fullscreen for the window showing video: the video's own window when it has one.
    pub(super) fn toggle_fullscreen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let SeparateVideo::Open { handle, .. } = &self.windowing.separate {
            let handle = *handle;
            // A key sent from the video's window arrives while that window is busy.
            cx.defer(move |cx| {
                handle.update(cx, VideoWindow::toggle_fullscreen).ok();
            });
            return;
        }
        self.windowing.prefs.toggle_fullscreen(window, cx);
    }

    /// Whether the window showing video is fullscreen (Escape leaves it).
    pub(super) fn video_is_fullscreen(&self, window: &Window) -> bool {
        match &self.windowing.separate {
            SeparateVideo::Open { fullscreen, .. } => fullscreen.get(),
            _ => window.is_fullscreen(),
        }
    }

    /// Sizes the window showing video to a `video`-sized picture ("Resize window to fit video").
    pub(super) fn fit_to_video(
        &mut self,
        video: (f32, f32),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let SeparateVideo::Open { handle, .. } = &self.windowing.separate {
            let handle = *handle;
            cx.defer(move |cx| {
                handle.update(cx, |this, window, cx| this.fit_video(video, window, cx)).ok();
            });
            return;
        }
        if self.showing_video(cx).is_none() {
            return;
        }
        let chrome = self.chrome(window, cx);
        if chrome.layout == Layout::Fullscreen {
            return;
        }
        let around =
            Surround { chrome: super::viewport::surround(chrome), min: app::MAIN_MIN_SIZE };
        self.windowing.prefs.fit_video(window, video, around, cx);
    }
}
