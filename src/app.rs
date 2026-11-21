//! Windows, routing and app-wide actions (S0.2).
//!
//! At startup the app opens either onboarding or the main window ([`cli::route`]).
//! Onboarding hands off by opening the main window *before* closing itself, because
//! with `QuitMode::LastWindowClosed` closing the only window quits the app.

pub mod cli;
mod display;
mod geometry;

use std::path::PathBuf;

use gpui::{
    App, DisplayId, SharedString, TitlebarOptions, WindowBounds, WindowHandle, WindowOptions,
    point, prelude::*, px, size,
};

use crate::keymap::actions::{CloseWindow, Quit, RunSetupAgain};
use crate::main_window::MainWindow;
use crate::onboarding::{self, Onboarding};
use crate::settings;
use cli::{Launch, Route};
use geometry::Rect;

pub use display::preferred_display;

/// The app id the desktop environment groups our windows by.
const APP_ID: &str = "org.redesignedvlc.app";

/// The main window's default size (S0.3.5).
const MAIN_DEFAULT_SIZE: (f32, f32) = (1280., 800.);
/// The main window's minimum size, unless the display is smaller.
const MAIN_MIN_SIZE: (f32, f32) = (800., 500.);
/// Onboarding shrinks to fit small screens, down to this size (S0.2.7); below it the
/// body scrolls.
const ONBOARDING_MIN_SIZE: (f32, f32) = (900., 640.);

/// Registers the app-wide action handlers. Key bindings and the macOS menu bar
/// come from the keymap ([`crate::keymap::init`]).
pub fn init(cx: &mut App) {
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &CloseWindow, cx| {
        if let Some(window) = cx.active_window() {
            cx.defer(move |cx| {
                window.update(cx, |_, window, _| window.remove_window()).ok();
            });
        }
    });
    cx.on_action(|_: &RunSetupAgain, cx| {
        let display = preferred_display(cx);
        cx.defer(move |cx| run_setup_again(display, cx));
    });
}

/// Opens the first window for this launch.
pub fn open_initial_window(launch: &Launch, cx: &mut App) {
    let onboarding_complete = settings::get(cx).local.onboarding_complete;
    let display = display::cursor_display(cx);
    let opened = match cli::route(onboarding_complete, launch) {
        Route::Onboarding => open_onboarding(display, cx).map(drop),
        Route::Main(files) => open_main_window(display, files, cx).map(drop),
        Route::Gallery => open_gallery(display, cx).map(drop),
    };
    if let Err(err) = opened {
        // Nothing to show; LastWindowClosed won't fire without a window, so quit.
        log::error!("couldn't open a window: {err:#}");
        cx.quit();
    }
    cx.activate(true);
}

/// Reopens a window when the app is activated with none open (macOS Dock icon).
pub fn reopen(cx: &mut App) {
    if cx.windows().is_empty() {
        open_initial_window(&Launch::default(), cx);
    }
}

/// Opens onboarding on `display`, at its design size or shrunk to fit.
pub fn open_onboarding(
    display: Option<DisplayId>,
    cx: &mut App,
) -> anyhow::Result<WindowHandle<Onboarding>> {
    let visible = visible_bounds(display, cx);
    let (fitted, min) = geometry::fit_to_display(
        (onboarding::WINDOW_WIDTH, onboarding::WINDOW_HEIGHT),
        ONBOARDING_MIN_SIZE,
        &visible,
    );
    let bounds = Rect::centered_in(fitted.0, fitted.1, &visible);
    let shrunk = fitted != (onboarding::WINDOW_WIDTH, onboarding::WINDOW_HEIGHT);
    let bounds = WindowBounds::Windowed(bounds.to_gpui());
    let handle = cx.open_window(
        WindowOptions {
            window_bounds: Some(bounds),
            // Resizable only when it had to shrink, so the user can make it larger
            // again on another display.
            is_resizable: shrunk,
            window_min_size: Some(size(px(min.0), px(min.1))),
            display_id: display,
            ..window_options(crate::window_chrome::TITLE_BAR_HEIGHT)
        },
        |window, cx| cx.new(|cx| Onboarding::new(window, cx)),
    )?;
    Ok(correct_dpi_drift(handle, bounds, cx))
}

/// Opens the main window where it was last saved (or centered at the default size),
/// with `files` to open in it.
pub fn open_main_window(
    display: Option<DisplayId>,
    files: Vec<PathBuf>,
    cx: &mut App,
) -> anyhow::Result<WindowHandle<MainWindow>> {
    let fallback = visible_bounds(display, cx);
    let displays: Vec<(DisplayId, Rect)> =
        cx.displays().iter().map(|d| (d.id(), Rect::from_gpui(d.visible_bounds()))).collect();
    let rects: Vec<Rect> = displays.iter().map(|(_, r)| *r).collect();
    let saved = settings::get(cx).local.window.main;
    let bounds = geometry::restore(&saved, &rects, &fallback, MAIN_MIN_SIZE).unwrap_or_else(|| {
        let w = MAIN_DEFAULT_SIZE.0.min(fallback.w);
        let h = MAIN_DEFAULT_SIZE.1.min(fallback.h);
        WindowBounds::Windowed(Rect::centered_in(w, h, &fallback).to_gpui())
    });
    // Create the window on the display that holds it, so Windows applies that
    // display's scale factor when placing it.
    let center = Rect::from_gpui(bounds.get_bounds()).center();
    let display = displays.iter().find(|(_, r)| r.contains(center)).map(|(id, _)| *id).or(display);
    let min_w = MAIN_MIN_SIZE.0.min(fallback.w);
    let min_h = MAIN_MIN_SIZE.1.min(fallback.h);
    let handle = cx.open_window(
        WindowOptions {
            window_bounds: Some(bounds),
            is_resizable: true,
            window_min_size: Some(size(px(min_w), px(min_h))),
            display_id: display,
            ..window_options(crate::main_window::TITLE_BAR_HEIGHT)
        },
        |window, cx| cx.new(|cx| MainWindow::new(files, window, cx)),
    )?;
    Ok(correct_dpi_drift(handle, bounds, cx))
}

/// Opens the component kit gallery (`--component-gallery`).
pub fn open_gallery(
    display: Option<DisplayId>,
    cx: &mut App,
) -> anyhow::Result<WindowHandle<crate::ui::gallery::Gallery>> {
    let visible = visible_bounds(display, cx);
    let bounds = Rect::centered_in(1200_f32.min(visible.w), 900_f32.min(visible.h), &visible);
    let bounds = WindowBounds::Windowed(bounds.to_gpui());
    let handle = cx.open_window(
        WindowOptions {
            window_bounds: Some(bounds),
            is_resizable: true,
            window_min_size: Some(size(px(400.), px(300.))),
            display_id: display,
            titlebar: Some(TitlebarOptions {
                title: Some(SharedString::from("VLC component gallery")),
                ..Default::default()
            }),
            app_id: Some(APP_ID.into()),
            ..Default::default()
        },
        |window, cx| cx.new(|cx| crate::ui::gallery::Gallery::new(window, cx)),
    )?;
    Ok(correct_dpi_drift(handle, bounds, cx))
}

/// Works around a GPUI bug on Windows: a window opened on a display whose scale
/// differs from the one Windows first creates it on is sized for the target
/// display, then scaled again by the `WM_DPICHANGED` that the move triggers. It
/// ends up 1.25x too large on a 125% display (and would grow on every launch once
/// its size is saved). Puts the size back to what was asked for.
///
/// Maximized and fullscreen windows aren't corrected: resizing would un-maximize
/// them, so their restore size can still be off on mixed-DPI setups.
fn correct_dpi_drift<V: 'static>(
    handle: WindowHandle<V>,
    requested: WindowBounds,
    cx: &mut App,
) -> WindowHandle<V> {
    let WindowBounds::Windowed(requested) = requested else {
        return handle;
    };
    handle
        .update(cx, |_, window, _| {
            if let WindowBounds::Windowed(actual) = window.window_bounds() {
                let drift = (actual.size.width - requested.size.width).abs()
                    + (actual.size.height - requested.size.height).abs();
                if drift > px(2.) {
                    log::debug!(
                        "window opened at {:?} instead of {:?}; resizing",
                        actual.size,
                        requested.size
                    );
                    window.resize(requested.size);
                }
            }
        })
        .ok();
    handle
}

/// "Run setup again" (S0.2.5): opens onboarding and closes the main window(s).
/// Finishing opens a fresh main window; closing onboarding early quits, and the
/// previous settings stay as they were.
pub fn run_setup_again(display: Option<DisplayId>, cx: &mut App) {
    if let Err(err) = open_onboarding(display, cx) {
        log::error!("couldn't open onboarding: {err:#}");
        return;
    }
    for handle in cx.windows() {
        if let Some(main) = handle.downcast::<MainWindow>() {
            main.update(cx, |_, window, _| window.remove_window()).ok();
        }
    }
}

/// Options shared by every window. `title_bar_height` centers the macOS traffic
/// lights (about 12px tall) in our title bar.
fn window_options(title_bar_height: f32) -> WindowOptions {
    WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: Some(SharedString::from("VLC")),
            // We draw our own title strip so the windows match the design on both
            // Windows and macOS.
            appears_transparent: true,
            traffic_light_position: Some(point(px(12.), px((title_bar_height - 8.) / 2.))),
        }),
        app_id: Some(APP_ID.into()),
        ..Default::default()
    }
}

/// The visible area (without taskbar/dock) of `display`, else the primary display.
fn visible_bounds(display: Option<DisplayId>, cx: &App) -> Rect {
    display
        .and_then(|id| cx.find_display(id))
        .or_else(|| cx.primary_display())
        .map_or(Rect::new(0., 0., 1280., 800.), |d| Rect::from_gpui(d.visible_bounds()))
}

/// Remembers a window's geometry whenever it moves or resizes (S0.2.9).
pub fn save_geometry_on_change<V: 'static>(
    window: &mut gpui::Window,
    cx: &mut Context<V>,
    slot: fn(&mut settings::Settings) -> &mut settings::WindowGeometry,
) {
    cx.observe_window_bounds(window, move |_, window, cx| {
        let geometry = geometry::to_saved(window.window_bounds());
        if geometry.width > 0. && geometry.height > 0. {
            settings::update(cx, |s| *slot(s) = geometry);
        }
    })
    .detach();
}
