//! Opening the video's own window (S5.4.1), where it was last left or else centered on the main
//! window's screen at the video's own size, shrunk to fit.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{App, DisplayId, Entity, SharedString, WindowHandle, WindowOptions, prelude::*};

use super::{Placement, correct_dpi_drift, visible_bounds, window_options};
use crate::main_window::MainWindow;
use crate::settings;
use crate::video::VideoView;
use crate::video_window::VideoWindow;
use crate::window_chrome::TITLE_BAR_HEIGHT;
use crate::window_prefs;

/// The smallest the video's window gets.
pub const VIDEO_WINDOW_MIN_SIZE: (f32, f32) = (320., 180.);

/// What the video's window shows and where it opens.
pub struct VideoWindowRequest {
    /// The main window's video view, lent to the new window.
    pub video: Entity<VideoView>,
    /// Kept up to date with whether the window is fullscreen, for the main window to read.
    pub fullscreen: Rc<Cell<bool>>,
    /// The video's size in pixels.
    pub video_size: (f32, f32),
    /// The main window, which keeps the controls and gets the keys.
    pub main: WindowHandle<MainWindow>,
    /// The main window's screen.
    pub display: Option<DisplayId>,
}

/// Opens the video's own window.
pub fn open_video_window(
    request: VideoWindowRequest,
    cx: &mut App,
) -> anyhow::Result<WindowHandle<VideoWindow>> {
    let VideoWindowRequest { video, fullscreen, video_size, main, display } = request;
    let area = visible_bounds(display, cx);
    let size = window_prefs::video_window_size(video_size, &area, VIDEO_WINDOW_MIN_SIZE);
    let saved = settings::get(cx).local.window.video;
    let placement = Placement::restore(&saved, size, VIDEO_WINDOW_MIN_SIZE, display, cx);
    let mut options = window_options(TITLE_BAR_HEIGHT);
    if let Some(titlebar) = options.titlebar.as_mut() {
        titlebar.title = Some(SharedString::from(tr!("VLC Video")));
    }
    let handle = cx.open_window(
        WindowOptions { is_resizable: true, ..placement.options(options) },
        |window, cx| cx.new(|cx| VideoWindow::new(video, main, fullscreen, window, cx)),
    )?;
    Ok(correct_dpi_drift(handle, placement.bounds, cx))
}
