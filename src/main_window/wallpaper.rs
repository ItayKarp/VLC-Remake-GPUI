//! "Play video as desktop wallpaper" (S8.16.3, Windows only): video plays in its own window, as
//! with "Play video inside main window" off, and that window goes behind the desktop icons on the
//! main display while this one keeps the controls.
//!
//! The window is checked every few seconds and put back if it isn't there yet (Explorer may be
//! slow to make its place). When Explorer restarts it takes the window with it; playback goes on
//! and a new window opens behind the icons (`video_window_closed`).
//! Turning the setting on or off while video plays closes the window and opens it again the new
//! way. A wallpaper has no fullscreen.

use std::time::Duration;

use gpui::{App, Context, Task, Window, WindowHandle};

use super::MainWindow;
use crate::native_window;
use crate::settings;
use crate::video_window::VideoWindow;

/// How often the wallpaper is checked.
const CHECK_EVERY: Duration = Duration::from_secs(2);

/// Whether video should play as the desktop's wallpaper.
pub(super) fn wanted(cx: &App) -> bool {
    cfg!(target_os = "windows") && settings::get(cx).prefs.video.desktop_wallpaper
}

impl MainWindow {
    /// Keeps the video's window `handle` behind the desktop icons until the task is dropped or
    /// the window is gone.
    pub(super) fn keep_as_wallpaper(
        handle: WindowHandle<VideoWindow>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        cx.spawn_in(window, async move |_, cx| {
            let keep = |_: &mut VideoWindow, window: &mut Window, _: &mut Context<VideoWindow>| {
                native_window::keep_as_wallpaper(window);
            };
            while handle.update(cx, keep).is_ok() {
                cx.background_executor().timer(CHECK_EVERY).await;
            }
        })
    }
}
