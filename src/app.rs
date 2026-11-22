//! Windows, routing and app-wide actions (S0.2), and launches (S0.6).
//!
//! At startup the app opens either onboarding or the main window ([`cli::route`]).
//! Later launches reach the running app through [`instance`] and [`open`].
//! Onboarding hands off by opening the main window *before* closing itself, because
//! with `QuitMode::LastWindowClosed` closing the only window quits the app.

pub mod cli;
mod display;
pub mod file_types;
pub mod geometry;
pub mod instance;
pub mod media;
mod not_responding;
pub mod open;
pub mod recent;
pub mod request;
pub mod screens;
mod settings_window;
mod video_window;

use gpui::{
    App, DisplayId, SharedString, TitlebarOptions, WindowBounds, WindowHandle, WindowOptions,
    point, prelude::*, px, size,
};

use crate::keymap::actions::{CloseWindow, OpenSettings, Quit, RunSetupAgain};
use crate::main_window::MainWindow;
use crate::onboarding::{self, Onboarding};
use crate::settings;
use crate::ui::toast::{Level, Toast};
use cli::Route;
use geometry::Rect;
use instance::{InboxReceiver, Role};
use request::{Launch, OpenRequest};

pub use display::preferred_display;
pub use settings_window::open_settings;
pub use video_window::{VIDEO_WINDOW_MIN_SIZE, VideoWindowRequest, open_video_window};

/// The app id the desktop environment groups our windows by.
const APP_ID: &str = "org.redesignedvlc.app";

/// The main window's default size (S0.3.5).
const MAIN_DEFAULT_SIZE: (f32, f32) = (1280., 800.);
/// The main window's minimum size, unless the display is smaller.
pub const MAIN_MIN_SIZE: (f32, f32) = (800., 500.);
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
    crate::settings_window::bind_keys(cx);
    cx.on_action(|_: &OpenSettings, cx| cx.defer(|cx| open_settings(None, cx)));
    // Clicking the now-playing notification brings VLC forward (S5.4.3).
    cx.on_system_notification_response(|response, cx| {
        if response.tag == crate::main_window::NOW_PLAYING_TAG {
            cx.activate(true);
        }
    });
    cx.on_action(|_: &RunSetupAgain, cx| {
        let display = preferred_display(cx);
        cx.defer(move |cx| run_setup_again(display, cx));
    });
}

/// Starts the UI for this process's [`Role`]: the first window, and receiving
/// launches forwarded by later processes.
pub fn start(launch: &Launch, role: Role, inbox: InboxReceiver, cx: &mut App) {
    match role {
        Role::NotResponding => {
            if let Err(err) = not_responding::open(cx) {
                tracing::error!("couldn't open a window: {err:#}");
                cx.quit();
            }
            cx.activate(true);
        }
        Role::Primary(server) => {
            cx.set_global(InstanceServer { _server: server });
            open_initial_window(launch, cx);
            open::receive(inbox, cx);
        }
        Role::Standalone | Role::Forwarded => {
            open_initial_window(launch, cx);
            open::receive(inbox, cx);
        }
    }
}

/// Keeps the instance lock and listener for the app's lifetime.
struct InstanceServer {
    _server: instance::Server,
}

impl gpui::Global for InstanceServer {}

/// Opens the first window for this launch.
pub fn open_initial_window(launch: &Launch, cx: &mut App) {
    let onboarding_complete = settings::get(cx).local.onboarding_complete;
    let display = display::cursor_display(cx);
    let opened = match cli::route(onboarding_complete, launch) {
        Route::Onboarding => open_onboarding(display, cx).map(drop),
        Route::Main(request) => open_main_window(display, request, cx).map(drop),
        Route::Gallery => open_gallery(display, cx).map(drop),
    };
    if let Err(err) = opened {
        // Nothing to show; LastWindowClosed won't fire without a window, so quit.
        tracing::error!("couldn't open a window: {err:#}");
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
/// opening `request` in it.
pub fn open_main_window(
    display: Option<DisplayId>,
    request: OpenRequest,
    cx: &mut App,
) -> anyhow::Result<WindowHandle<MainWindow>> {
    let saved = settings::get(cx).local.window.main;
    let placement = Placement::restore(&saved, MAIN_DEFAULT_SIZE, MAIN_MIN_SIZE, display, cx);
    let handle = cx.open_window(
        WindowOptions {
            is_resizable: true,
            ..placement.options(window_options(crate::main_window::TITLE_BAR_HEIGHT))
        },
        |window, cx| cx.new(|cx| MainWindow::new(request, window, cx)),
    )?;
    Ok(correct_dpi_drift(handle, placement.bounds, cx))
}

/// Where a remembered window opens: its bounds, minimum size and display.
struct Placement {
    bounds: WindowBounds,
    min_size: (f32, f32),
    display: Option<DisplayId>,
}

impl Placement {
    /// Where `saved` put the window, or centered on `display` at `default` size, with `min`
    /// shrunk to fit the display.
    fn restore(
        saved: &settings::WindowGeometry,
        default: (f32, f32),
        min: (f32, f32),
        display: Option<DisplayId>,
        cx: &App,
    ) -> Self {
        // "Remember window size and position" off: every window opens centered (S5.4.1).
        let forgotten = settings::WindowGeometry::default();
        let remembered = settings::get(cx).prefs.interface.remember_window_geometry;
        let saved = if remembered { saved } else { &forgotten };
        let fallback = visible_bounds(display, cx);
        let displays: Vec<(DisplayId, Rect)> =
            cx.displays().iter().map(|d| (d.id(), Rect::from_gpui(d.visible_bounds()))).collect();
        let rects: Vec<Rect> = displays.iter().map(|(_, r)| *r).collect();
        let bounds = geometry::restore(saved, &rects, &fallback, min).unwrap_or_else(|| {
            let (w, h) = (default.0.min(fallback.w), default.1.min(fallback.h));
            WindowBounds::Windowed(Rect::centered_in(w, h, &fallback).to_gpui())
        });
        // Create the window on the display that holds it, so Windows applies that
        // display's scale factor when placing it.
        let center = Rect::from_gpui(bounds.get_bounds()).center();
        let display =
            displays.iter().find(|(_, r)| r.contains(center)).map(|(id, _)| *id).or(display);
        Self { bounds, min_size: (min.0.min(fallback.w), min.1.min(fallback.h)), display }
    }

    /// `options` with this placement.
    fn options(&self, options: WindowOptions) -> WindowOptions {
        WindowOptions {
            window_bounds: Some(self.bounds),
            window_min_size: Some(size(px(self.min_size.0), px(self.min_size.1))),
            display_id: self.display,
            ..options
        }
    }
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
                    tracing::debug!(
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

/// "Run setup again" (S0.2.5, S5.3.5): opens onboarding and closes the main and Settings
/// windows. Finishing or cancelling opens a fresh main window; cancelling keeps the settings as
/// they were.
pub fn run_setup_again(display: Option<DisplayId>, cx: &mut App) {
    if let Err(err) = open_onboarding(display, cx) {
        tracing::error!("couldn't open onboarding: {err:#}");
        show_error_in_main_windows("Setup couldn't be opened. See the log for details.", cx);
        return;
    }
    for handle in cx.windows() {
        let replaced = handle.downcast::<MainWindow>().is_some()
            || handle.downcast::<crate::settings_window::SettingsWindow>().is_some();
        if replaced {
            handle.update(cx, |_, window, _| window.remove_window()).ok();
        }
    }
}

/// Shows an error toast in every main window, for failures the user caused from one.
fn show_error_in_main_windows(message: &'static str, cx: &mut App) {
    let mains = cx.windows().into_iter().filter_map(|handle| handle.downcast::<MainWindow>());
    for main in mains {
        main.update(cx, |main, _, cx| main.show_toast(Toast::new(Level::Error, message), cx)).ok();
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

/// Remembers a window's geometry whenever it moves or resizes (S0.2.9), while "Remember window
/// size and position" is on (S5.4.1).
pub fn save_geometry_on_change<V: 'static>(
    window: &mut gpui::Window,
    cx: &mut Context<V>,
    slot: fn(&mut settings::Settings) -> &mut settings::WindowGeometry,
) {
    cx.observe_window_bounds(window, move |_, window, cx| {
        let geometry = geometry::to_saved(window.window_bounds());
        let remembered = settings::get(cx).prefs.interface.remember_window_geometry;
        if remembered && geometry.width > 0. && geometry.height > 0. {
            settings::update(cx, |s| *slot(s) = geometry);
        }
    })
    .detach();
}
