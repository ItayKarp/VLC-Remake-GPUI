//! Opening the Settings window (S5.2.1): from the main menu, the macOS app menu, Ctrl+P or
//! Cmd+`,`, and the settings panel's ⚙ Open Advanced Settings.
//!
//! **Decision:** a separate OS window rather than a modal in the main window, so it can sit
//! beside the player while tuning video. There is only ever one: opening it again, from anywhere,
//! brings the open one to the front. It opens where it was last left, on the page it last showed.

use gpui::{App, Global, SharedString, WindowHandle, WindowOptions, prelude::*};

use super::{Placement, correct_dpi_drift, display, window_options};
use crate::settings::{self, registry::Category};
use crate::settings_window::SettingsWindow;
use crate::window_chrome::TITLE_BAR_HEIGHT;

/// The window's size in the designs (`docs/ui/settings/screens/*`, drawn at 2x).
const DEFAULT_SIZE: (f32, f32) = (1000., 830.);
/// The smallest the window gets: narrow enough that the sidebar shows icons only.
const MIN_SIZE: (f32, f32) = (520., 420.);

/// The Settings window, while one is open.
struct OpenWindow(WindowHandle<SettingsWindow>);

impl Global for OpenWindow {}

/// Brings Settings to the front, opening it if needed, on `category` or else the page it showed
/// last.
pub fn open_settings(category: Option<Category>, cx: &mut App) {
    if let Some(OpenWindow(handle)) = cx.try_global::<OpenWindow>() {
        let handle = *handle;
        let focused = handle.update(cx, |view, window, cx| {
            if let Some(category) = category {
                view.select(category, window, cx);
            }
            window.activate_window();
        });
        if focused.is_ok() {
            return;
        }
    }
    match open_window(category, cx) {
        Ok(handle) => cx.set_global(OpenWindow(handle)),
        Err(err) => tracing::error!("couldn't open the Settings window: {err:#}"),
    }
}

fn open_window(
    category: Option<Category>,
    cx: &mut App,
) -> anyhow::Result<WindowHandle<SettingsWindow>> {
    let saved = settings::get(cx).local.window.settings;
    let display = display::preferred_display(cx);
    let placement = Placement::restore(&saved, DEFAULT_SIZE, MIN_SIZE, display, cx);
    let mut options = window_options(TITLE_BAR_HEIGHT);
    if let Some(titlebar) = options.titlebar.as_mut() {
        titlebar.title = Some(SharedString::from("Settings"));
    }
    let category = category.unwrap_or(settings::get(cx).local.settings_category);
    let handle = cx.open_window(
        WindowOptions { is_resizable: true, ..placement.options(options) },
        |window, cx| cx.new(|cx| SettingsWindow::new(category, window, cx)),
    )?;
    Ok(correct_dpi_drift(handle, placement.bounds, cx))
}
