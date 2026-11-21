//! The VLC desktop app: starts GPUI, initializes each service, and opens the first
//! window.

mod app;
mod assets;
mod keymap;
mod logging;
mod main_window;
mod onboarding;
mod settings;
mod theme;
mod ui;
mod window_chrome;

use gpui::{App, QuitMode};
use gpui_platform::application;

use crate::app::cli::Launch;

fn main() {
    logging::init();
    let launch = Launch::from_env();
    let application =
        application().with_assets(assets::Assets).with_quit_mode(QuitMode::LastWindowClosed);
    application.on_reopen(app::reopen);
    application.run(move |cx: &mut App| {
        settings::init(cx);
        theme::init(cx);
        ui::init(cx);
        keymap::init(cx);
        app::init(cx);
        app::open_initial_window(&launch, cx);
    });
}
