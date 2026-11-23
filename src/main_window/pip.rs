//! Picture in Picture (S8.5): the player popped out into a small window above the others
//! ([`PipWindow`]), with the docked card's controls, while this window keeps everything else.
//!
//! The View menu's and the video menu's Picture in Picture pops it out while something plays,
//! and again pops it back in. While it shows, the video plays there: this window shows what
//! plays in its empty state, the docked card in full library mode gives way to it, and video
//! that would play in a window of its own (S5.4.1) plays there instead. Its ✕ stops playback;
//! the window closes with it and this window comes forward, so VLC always has a window to come
//! back to. Its pop-in, a double click on it and Picture in Picture again bring the player back
//! here, leaving full library mode. It closes a moment after playback stops or fails, unless
//! the next item plays; once the media ended it stays, with Replay.
//!
//! With Settings → Interface "Start in compact mode", the first media played after launch plays
//! in it, this window minimized.

use std::time::Duration;

use gpui::{App, Context, Entity, Subscription, Window, WindowHandle};

use super::MainWindow;
use super::miniplayer::progress;
use super::video_menu::NOTHING_PLAYING;
use crate::app::{self, PipWindowRequest};
use crate::db::metadata::MediaKind;
use crate::engine::state::PlaybackState;
use crate::engine::values::SeekTarget;
use crate::library::folder_view::kind_by_extension;
use crate::pip_window::PipWindow;
use crate::settings;
use crate::ui::player_overlay::TransportState;

/// How long the window stays after it is no longer needed, in case it is again.
const CLOSE_DELAY: Duration = Duration::from_millis(500);
/// The aspect ratio of media without a picture.
const DEFAULT_ASPECT: f32 = 16. / 9.;

/// The Picture in Picture window.
#[derive(Default)]
pub enum Pip {
    /// The player plays in this window.
    #[default]
    Closed,
    /// The window is being opened.
    Opening,
    /// The window is open, and the subscription noticing it close.
    Open { handle: WindowHandle<PipWindow>, _closed: Subscription },
}

impl Pip {
    /// Whether the player plays in the Picture in Picture window.
    pub fn is_shown(&self) -> bool {
        !matches!(self, Self::Closed)
    }
}

/// How this launch plays its first media.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Launch {
    /// In the main window, or it was played already.
    #[default]
    Normal,
    /// In the Picture in Picture window ("Start in compact mode").
    Compact,
}

/// What the Picture in Picture window asks of the main window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PipCommand {
    /// ✕: stop playback, which closes the window.
    Stop,
    /// Back to the player in the main window.
    PopIn,
    /// Previous.
    Previous,
    /// Play/Pause, or Replay once ended.
    PlayPause,
    /// Next.
    Next,
    /// Go to this fraction of the media.
    Seek(f32),
}

/// What the Picture in Picture window draws.
#[derive(Clone, Copy, Debug)]
pub struct PipView {
    /// What the transport can do.
    pub transport: TransportState,
    /// How far playback is, from 0 to 1.
    pub progress: f32,
    /// The media has a picture to show.
    pub video: bool,
    /// Without a picture, it's a song rather than a video still opening.
    pub song: bool,
}

impl MainWindow {
    /// Pops the player out, or back in; says why not over the video when nothing plays.
    pub(super) fn toggle_pip(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.windowing.pip.is_shown() {
            self.pop_in(window, cx);
        } else if self.wants_pip(cx) {
            self.open_pip(window, cx);
        } else {
            self.show_osd(NOTHING_PLAYING, cx);
        }
    }

    /// What the Picture in Picture window draws, while something plays or has just ended.
    pub fn pip_view(&self, cx: &App) -> Option<PipView> {
        let player = self.loaded_player(cx)?;
        let read = player.read(cx);
        let song = read.media().and_then(|media| media.as_path()).map(kind_by_extension);
        Some(PipView {
            transport: self.transport_state(cx),
            progress: progress(read.position(), read.duration()),
            video: read.video_size().is_some(),
            song: song == Some(MediaKind::Audio),
        })
    }

    /// Does what the Picture in Picture window asks.
    pub fn pip_command(
        &mut self,
        command: PipCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match command {
            PipCommand::Stop => {
                self.stop(window, cx);
                self.pop_in_without_leaving_library(window, cx);
            }
            PipCommand::PopIn => self.pop_in(window, cx),
            PipCommand::Previous => self.previous(window, cx),
            PipCommand::PlayPause => self.play_pause(window, cx),
            PipCommand::Next => self.next(window, cx),
            PipCommand::Seek(fraction) => {
                let Some(player) = self.loaded_player(cx) else { return };
                let Some(duration) = player.read(cx).duration() else { return };
                self.seek_player(SeekTarget::Absolute(duration.mul_f32(fraction)), cx);
            }
        }
    }

    /// Whether the window has something to show: media playing, paused or just ended.
    fn wants_pip(&self, cx: &App) -> bool {
        self.loaded_player(cx).is_some_and(|player| {
            let state = player.read(cx).state();
            state.is_active() || state == PlaybackState::Ended
        })
    }

    /// The picture's width over its height.
    fn pip_aspect(&self, cx: &App) -> f32 {
        let size = self.loaded_player(cx).and_then(|player| player.read(cx).video_size());
        size.filter(|size| size.width > 0 && size.height > 0)
            .map_or(DEFAULT_ASPECT, |size| size.width as f32 / size.height as f32)
    }

    fn open_pip(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(video) = self.video.clone() else { return };
        let Some(main) = window.window_handle().downcast::<Self>() else { return };
        // Video in a window of its own comes back first, so this window can lend it on.
        self.close_video_window(window, cx);
        video.update(cx, |video, _| video.leave(window));
        let request = PipWindowRequest {
            video,
            aspect: self.pip_aspect(cx),
            main: (main, cx.entity().downgrade()),
            display: window.display(cx).map(|display| display.id()),
        };
        self.windowing.pip = Pip::Opening;
        cx.spawn_in(window, async move |this, cx| {
            let opened = cx.update(|_, cx| app::open_pip_window(request, cx));
            this.update_in(cx, |this, window, cx| match opened {
                Ok(Ok(opened)) => this.adopt_pip(opened, window, cx),
                Ok(Err(err)) | Err(err) => {
                    tracing::error!("couldn't open the Picture in Picture window: {err:#}");
                    this.pip_gone(window, cx);
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Follows the opened window: closed some other way, the player comes back here.
    fn adopt_pip(
        &mut self,
        (handle, entity): (WindowHandle<PipWindow>, Entity<PipWindow>),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let closed = cx.observe_release_in(&entity, window, |this, _, window, cx| {
            if matches!(this.windowing.pip, Pip::Open { .. }) {
                this.pip_gone(window, cx);
            }
        });
        self.windowing.pip = Pip::Open { handle, _closed: closed };
        // Playback may have stopped while it opened.
        self.sync_pip(window, cx);
    }

    /// Back to the player here, out of full library mode, this window in front.
    fn pop_in(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pop_in_without_leaving_library(window, cx);
        self.leave_full_library(window, cx);
    }

    /// Closes the window, the video playing here again and this window in front.
    fn pop_in_without_leaving_library(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Pip::Open { handle, .. } = std::mem::take(&mut self.windowing.pip) else { return };
        cx.defer(move |cx| {
            handle
                .update(cx, |pip, window, cx| {
                    pip.release_video(window, cx);
                    window.remove_window();
                })
                .ok();
        });
        self.pip_gone(window, cx);
        window.activate_window();
    }

    /// The window is gone: the video plays here again, or in its own window if set so.
    fn pip_gone(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.windowing.pip = Pip::Closed;
        self.video_back_home(window, cx);
        self.sync_video_window(window, cx);
    }

    /// Closes the window a moment after playback stopped or failed, unless it plays again by
    /// then (the queue going on to the next item). Call it when the playback state changes.
    pub(super) fn sync_pip(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.windowing.pip, Pip::Open { .. }) || self.wants_pip(cx) {
            return;
        }
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(CLOSE_DELAY).await;
            this.update_in(cx, Self::close_pip_unless_wanted).ok();
        })
        .detach();
    }

    fn close_pip_unless_wanted(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.wants_pip(cx) {
            self.pop_in_without_leaving_library(window, cx);
        }
    }

    /// The video's size changed: the window takes its aspect ratio.
    pub(super) fn refit_pip(&self, cx: &mut Context<Self>) {
        let Pip::Open { handle, .. } = &self.windowing.pip else { return };
        let (handle, aspect) = (*handle, self.pip_aspect(cx));
        cx.defer(move |cx| {
            handle.update(cx, |pip, window, cx| pip.set_aspect(aspect, window, cx)).ok();
        });
    }

    /// Remembers whether this launch starts in compact mode (Settings → Interface).
    pub(super) fn plan_compact_start(&mut self, cx: &App) {
        if settings::get(cx).prefs.interface.start_in_compact_mode {
            self.windowing.launch = Launch::Compact;
        }
    }

    /// The first media played after a compact-mode launch pops out, this window minimized.
    pub(super) fn start_compact(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let launch = std::mem::take(&mut self.windowing.launch);
        if launch == Launch::Normal || self.windowing.pip.is_shown() {
            return;
        }
        self.open_pip(window, cx);
        window.minimize_window();
    }
}
