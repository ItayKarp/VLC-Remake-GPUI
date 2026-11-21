//! The VLC desktop app: hands the launch to a running instance if there is one,
//! otherwise starts GPUI, initializes each service, and opens the first window.

mod app;
mod assets;
mod diagnostics;
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

use crate::app::instance::{self, Role};
use crate::app::request::Launch;

fn main() {
    diagnostics::init();
    let launch = Launch::from_env();
    let (inbox, receiver) = instance::inbox();
    let role = instance::acquire(&launch, &inbox);
    if matches!(role, Role::Forwarded) {
        return;
    }
    let application =
        application().with_assets(assets::Assets).with_quit_mode(QuitMode::LastWindowClosed);
    application.on_reopen(app::reopen);
    application.on_open_urls(move |urls| inbox.post_urls(&urls));
    application.run(move |cx: &mut App| {
        settings::init(cx);
        diagnostics::follow_settings(cx);
        theme::init(cx);
        ui::init(cx);
        keymap::init(cx);
        app::init(cx);
        app::start(&launch, role, receiver, cx);
    });
}
