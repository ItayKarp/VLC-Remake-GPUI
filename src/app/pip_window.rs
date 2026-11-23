//! Opening the Picture in Picture window (S8.5): where it was last left on the main window's
//! display, else in that display's bottom-right corner, at the video's aspect ratio.

use anyhow::Context as _;
use gpui::{App, DisplayId, Entity, WindowHandle, WindowOptions, prelude::*, px, size};

use super::geometry::Rect;
use super::{visible_bounds, window_options};
use crate::main_window::MainWindow;
use crate::pip_window::{PipWindow, placement};
use crate::settings;
use crate::video::VideoView;

/// What the Picture in Picture window shows and where it opens.
pub struct PipWindowRequest {
    /// The main window's video view, lent to the new window.
    pub video: Entity<VideoView>,
    /// The picture's width over its height.
    pub aspect: f32,
    /// The main window, which plays the media and gets the keys, and its view.
    pub main: (WindowHandle<MainWindow>, gpui::WeakEntity<MainWindow>),
    /// The main window's display.
    pub display: Option<DisplayId>,
}

/// Opens the Picture in Picture window: its handle, and its view.
pub fn open_pip_window(
    request: PipWindowRequest,
    cx: &mut App,
) -> anyhow::Result<(WindowHandle<PipWindow>, Entity<PipWindow>)> {
    let PipWindowRequest { video, aspect, main, display } = request;
    let display = display.and_then(|id| cx.find_display(id)).or_else(|| cx.primary_display());
    let area = visible_bounds(display.as_ref().map(|display| display.id()), cx);
    let screen = display.as_ref().map_or(area, |display| Rect::from_gpui(display.bounds()));
    let remembered = settings::get(cx).prefs.interface.remember_window_geometry;
    let places = if remembered { settings::get(cx).local.pip_places.clone() } else { Vec::new() };
    let bounds = placement::place(&places, screen, &area, aspect);
    let options = WindowOptions {
        window_bounds: Some(gpui::WindowBounds::Windowed(bounds.to_gpui())),
        window_min_size: Some(size(px(placement::MIN_SIZE.0), px(placement::MIN_SIZE.1))),
        display_id: display.map(|display| display.id()),
        is_resizable: true,
        is_minimizable: false,
        // The window draws no title bar: its controls are at the top, and it moves itself.
        app_owns_titlebar_drag: true,
        // It usually plays while another app is active: no slower frames then.
        inactive_frame_interval: None,
        focus: false,
        ..window_options(0.)
    };
    let mut view = None;
    let handle = cx.open_window(options, |window, cx| {
        let built = cx.new(|cx| PipWindow::new(video, main, aspect, window, cx));
        view = Some(built.clone());
        built
    })?;
    let view = view.context("the window was opened without its view")?;
    Ok((handle, view))
}
