//! The VLC desktop app: hands the launch to a running instance if there is one,
//! otherwise starts GPUI, initializes each service, and opens the first window.

mod app;
mod assets;
mod cursor;
mod db;
mod diagnostics;
#[allow(dead_code, reason = "the S1.2 engine; the player screen (S2) is its first caller")]
mod engine;
mod history;
mod keymap;
mod logging;
mod main_window;
mod onboarding;
mod opening;
mod power;
mod queue;
#[cfg(test)]
mod screenshot_test;
mod session;
mod settings;
mod theme;
mod ui;
mod video;
mod window_chrome;

use gpui::{App, QuitMode};
use gpui_platform::application;

use crate::app::instance::{self, Role};
use crate::app::request::Launch;

fn main() {
    // SAFETY: nothing else runs yet, so no thread can read the environment meanwhile.
    unsafe { engine::prepare_environment() };
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
        history::init(cx);
        session::init(cx);
        diagnostics::follow_settings(cx);
        theme::init(cx);
        ui::init(cx);
        keymap::init(cx);
        app::init(cx);
        app::start(&launch, role, receiver, cx);
    });
}
