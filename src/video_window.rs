//! The video's own window (S5.4.1): with Settings → Interface "Play video inside main window" off,
//! video plays here while the main window keeps the controls and says what plays.
//!
//! The main window opens and closes it (`main_window/video_window.rs`) and lends it its
//! [`VideoView`]. It is only the picture: dragging it moves the window (macOS and Linux; the
//! strip along the top on Windows), a double click goes fullscreen on the "Open fullscreen on"
//! screen, and keys go to the main window, so the player's keys work from here too. Closing it
//! stops playback.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyWindowHandle, Context, Entity, FocusHandle, KeyDownEvent, KeyUpEvent, Window, WindowHandle,
    black, div, prelude::*,
};

use crate::app;
use crate::keymap::actions::ToggleFullscreen;
use crate::main_window::MainWindow;
use crate::settings;
use crate::video::VideoView;
use crate::window_chrome;
use crate::window_prefs::{Surround, WindowPrefs};

/// The height of the strip along the top that moves the window.
const DRAG_STRIP_HEIGHT: f32 = 32.;

/// The video's own window.
pub struct VideoWindow {
    video: Entity<VideoView>,
    /// The main window, which the keys go to.
    main: WindowHandle<MainWindow>,
    focus_handle: FocusHandle,
    prefs: WindowPrefs,
    /// Whether the window is fullscreen, shared with the main window.
    fullscreen: Rc<Cell<bool>>,
}

impl VideoWindow {
    /// The window showing `video`, sending keys to `main` and keeping `fullscreen` up to date.
    pub fn new(
        video: Entity<VideoView>,
        main: WindowHandle<MainWindow>,
        fullscreen: Rc<Cell<bool>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe_window_bounds(window, |_, _, cx| cx.notify()).detach();
        cx.observe_global::<settings::SettingsStore>(|_, cx| cx.notify()).detach();
        app::save_geometry_on_change(window, cx, |s| &mut s.local.window.video);
        video.update(cx, |video, cx| video.arrive(window, cx));
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        Self { video, main, focus_handle, prefs: WindowPrefs::default(), fullscreen }
    }

    /// Switches fullscreen, on the "Open fullscreen on" screen.
    pub fn toggle_fullscreen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.prefs.toggle_fullscreen(window, cx);
    }

    /// Sizes the window to a `video`-sized picture, as "Resize window to fit video" says.
    pub fn fit_video(&mut self, video: (f32, f32), window: &mut Window, cx: &mut Context<Self>) {
        let around = Surround { chrome: (0., 0.), min: app::VIDEO_WINDOW_MIN_SIZE };
        self.prefs.fit_video(window, video, around, cx);
    }

    /// Gives the video back: frees what this window keeps for its picture.
    pub fn release_video(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.video.update(cx, |video, _| video.leave(window));
    }

    /// A key was released: the main window's held-key gate must hear of it, or it would take
    /// the next press of the same key for a held key's repeat.
    fn forward_key_up(&mut self, _: &KeyUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.main.update(cx, |main, _, _| main.release_keys()).ok();
    }

    /// Sends a key the main window's player understands there.
    fn forward_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let keystroke = event.keystroke.clone();
        // Through the untyped handle: the typed one holds the main view while the key dispatches,
        // and the main window may draw itself during the dispatch.
        let main: AnyWindowHandle = self.main.into();
        let sent = main.update(cx, |_, window, cx| window.dispatch_keystroke(keystroke, cx));
        if sent.is_ok() {
            cx.stop_propagation();
        }
    }
}

impl Render for VideoWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.prefs.sync(window, cx);
        self.fullscreen.set(window.is_fullscreen());
        let picture = div().id("video-window-picture").size_full().child(self.video.clone());
        let picture =
            if window.is_fullscreen() { picture } else { window_chrome::drags_window(picture) };
        div()
            .id("video-window")
            .relative()
            .size_full()
            .bg(black())
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &ToggleFullscreen, window, cx| {
                this.toggle_fullscreen(window, cx);
            }))
            .on_key_down(cx.listener(Self::forward_key))
            .on_key_up(cx.listener(Self::forward_key_up))
            .child(picture)
            .children(
                (!window.is_fullscreen())
                    .then(|| window_chrome::drag_bar("video-window-drag", DRAG_STRIP_HEIGHT)),
            )
    }
}
