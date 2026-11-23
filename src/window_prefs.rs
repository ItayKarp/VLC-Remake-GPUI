//! How VLC's video windows (the player, and the video's own window) follow Settings → Interface's
//! Window section (S5.4.1): staying above other windows, going fullscreen on the chosen screen,
//! and taking the video's size.
//!
//! - "Keep window above other windows" applies except in fullscreen, so a fullscreen video never
//!   covers the system's own dialogs; leaving fullscreen puts it back above.
//! - "Open fullscreen on" moves the window to the chosen screen just before fullscreen starts,
//!   and back where it was once fullscreen ends. A screen that isn't connected means the current
//!   one.
//! - "Resize window to fit video" sizes the window to the video, shrunk to the screen's work area
//!   and moved back onto it; never while fullscreen or maximized.

mod fit;

pub use fit::kept_inside;

use gpui::{App, Pixels, Point, Window, px, size};

use crate::app::geometry::Rect;
use crate::app::screens;
use crate::native_window;
use crate::settings::{self, FitVideo};

/// One window's state for the Window settings.
#[derive(Default)]
pub struct WindowPrefs {
    /// What the window's level was last set to.
    above: Option<bool>,
    /// Fullscreen moved the window to another screen.
    moved: FullscreenMove,
    /// The window was fitted to a video already.
    fitted: bool,
}

/// Where a window that fullscreen moved to another screen goes back to.
#[derive(Clone, Copy, Default)]
enum FullscreenMove {
    /// Nothing was moved.
    #[default]
    None,
    /// Moved, and fullscreen is starting.
    Entering(Point<Pixels>),
    /// Moved and fullscreen: goes back here when fullscreen ends.
    Fullscreen(Point<Pixels>),
}

/// The size a fitted window has around its video, and the smallest it may be.
#[derive(Clone, Copy, Debug)]
pub struct Surround {
    /// The bars and insets around the picture, in width and height.
    pub chrome: (f32, f32),
    /// The window's minimum size.
    pub min: (f32, f32),
}

impl WindowPrefs {
    /// Brings the window's level up to date and moves it back from fullscreen on another screen.
    /// Call it on every render; it asks the OS only when something changed.
    pub fn sync(&mut self, window: &mut Window, cx: &App) {
        let fullscreen = window.is_fullscreen();
        let above = settings::get(cx).prefs.interface.keep_above_other_windows && !fullscreen;
        if self.above != Some(above) {
            self.above = Some(above);
            if !native_window::set_keep_above(window, above) && above {
                tracing::info!("keeping a window above others isn't supported here");
            }
        }
        self.moved = match self.moved {
            FullscreenMove::Entering(origin) if fullscreen => FullscreenMove::Fullscreen(origin),
            FullscreenMove::Fullscreen(origin) if !fullscreen => {
                native_window::move_to(window, origin, cx);
                FullscreenMove::None
            }
            moved => moved,
        };
    }

    /// Switches fullscreen, going to the "Open fullscreen on" screen first when entering it.
    pub fn toggle_fullscreen(&mut self, window: &mut Window, cx: &App) {
        if !window.is_fullscreen() {
            // macOS won't take a floating window fullscreen: drop to the normal level first.
            if self.above == Some(true) {
                native_window::set_keep_above(window, false);
                self.above = Some(false);
            }
            self.move_to_fullscreen_screen(window, cx);
        }
        window.toggle_fullscreen();
    }

    fn move_to_fullscreen_screen(&mut self, window: &Window, cx: &App) {
        let chosen = &settings::get(cx).local.fullscreen_display;
        let Some(screen) = screens::find(chosen, cx) else { return };
        if window.display(cx).is_some_and(|on| on.id() == screen.id()) {
            return;
        }
        let from = window.bounds().origin;
        if native_window::move_to(window, screen.visible_bounds().origin, cx) {
            self.moved = FullscreenMove::Entering(from);
        }
    }

    /// Sizes the window to a `video`-sized picture, as "Resize window to fit video" says.
    pub fn fit_video(
        &mut self,
        window: &mut Window,
        video: (f32, f32),
        around: Surround,
        cx: &App,
    ) {
        let wanted = match settings::get(cx).prefs.interface.resize_to_fit_video {
            FitVideo::Never => false,
            FitVideo::FirstVideo => !self.fitted,
            FitVideo::EveryVideo => true,
        };
        if !wanted || video.0 < 1. || video.1 < 1. {
            return;
        }
        self.fitted = true;
        if window.is_fullscreen() || window.is_maximized() {
            return;
        }
        let Some(display) = window.display(cx) else { return };
        let area = Rect::from_gpui(display.visible_bounds());
        let (w, h) = fit::window_size(video, around.chrome, &area, around.min);
        let now = Rect::from_gpui(window.bounds());
        window.resize(size(px(w), px(h)));
        let placed = kept_inside(Rect::new(now.x, now.y, w, h), &area);
        // AppKit keeps a resized window's bottom left corner; the top left one stays here.
        native_window::move_to(window, placed.to_gpui().origin, cx);
    }
}

/// The size a new video window opens at on `area`: the video's own, shrunk to fit.
pub fn video_window_size(video: (f32, f32), area: &Rect, min: (f32, f32)) -> (f32, f32) {
    fit::window_size(video, (0., 0.), area, min)
}
