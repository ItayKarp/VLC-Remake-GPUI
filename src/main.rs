//! The VLC desktop app: hands the launch to a running instance if there is one,
//! otherwise starts GPUI, initializes each service, and opens the first window.

// Shipped Windows builds start without a console window (S9.4.1); debug builds keep theirs for
// the log.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[macro_use]
mod i18n;

mod app;
mod assets;
mod capture;
mod convert;
mod cursor;
mod db;
mod diagnostics;
mod discs;
mod effects;
#[allow(dead_code, reason = "the S1.2 engine; the player screen (S2) is its first caller")]
mod engine;
#[cfg(test)]
mod fuzz;
mod history;
mod keymap;
mod library;
mod logging;
mod main_window;
mod native_window;
mod navigation;
mod network;
mod onboarding;
mod opening;
mod perf;
mod pip_window;
mod player_layout;
mod playlists;
mod podcasts;
mod power;
mod privacy;
mod queue;
mod recording;
mod remote;
#[cfg(test)]
mod screenshot_test;
mod scrobble;
mod session;
mod settings;
mod settings_view;
mod settings_window;
mod snapshot;
mod subtitle_links;
mod theme;
mod tray;
mod ui;
mod updates;
mod video;
mod video_window;
mod window_chrome;
mod window_prefs;

use gpui::{App, QuitMode};
use gpui_platform::application;

use crate::app::instance::{self, Role};
use crate::app::request::Launch;

fn main() {
    perf::start();
    // SAFETY: nothing else runs yet, so no thread can read the environment meanwhile.
    unsafe { engine::prepare_environment() };
    diagnostics::init();
    let launch = Launch::from_env();
    let (inbox, receiver) = instance::inbox();
    let role = instance::acquire(&launch, &inbox);
    if matches!(role, Role::Forwarded) {
        return;
    }
    // An update that failed to start twice is rolled back here, and the previous version
    // started in this one's place (S9.5.2).
    let Some(role) = updates::launched(role) else { return };
    let application =
        application().with_assets(assets::Assets).with_quit_mode(QuitMode::LastWindowClosed);
    application.on_reopen(app::reopen);
    application.on_open_urls(move |urls| inbox.post_urls(&urls));
    application.run(move |cx: &mut App| {
        settings::init(cx);
        i18n::init(cx);
        history::init(cx);
        library::init(cx);
        playlists::init(cx);
        convert::init(cx);
        network::init(cx);
        podcasts::init(cx);
        scrobble::init(cx);
        updates::init(cx);
        session::init(cx);
        diagnostics::follow_settings(cx);
        perf::init(cx);
        theme::init(cx);
        ui::init(cx);
        keymap::init(cx);
        app::init(cx);
        app::start(&launch, role, receiver, cx);
    });
}
