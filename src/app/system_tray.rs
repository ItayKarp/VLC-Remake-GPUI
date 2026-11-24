//! The system tray icon's owner (S8.16.1): shows or takes away the icon as Settings → Interface
//! "Show VLC in the system tray" says, and runs what its menu asks. Play/Pause, Previous, Next and
//! Show VLC go to the main window; with none open (macOS keeps running without windows), Show VLC
//! opens one and the others do nothing. Quit asks about unfinished conversions first, as the app
//! menu's Quit does.

use futures::StreamExt;
use futures::channel::mpsc::unbounded;
use gpui::{App, Global, Task};

use super::request::Launch;
use crate::i18n::Language;
use crate::main_window::MainWindow;
use crate::settings;
use crate::tray::{CommandSender, TrayCommand, TrayIcon};

/// The icon, while it shows, the language its menu is in, and the task running its commands.
struct SystemTray {
    sender: CommandSender,
    icon: Option<TrayIcon>,
    language: Option<Language>,
    _commands: Task<()>,
}

impl Global for SystemTray {}

/// Shows the icon if Settings asks for it, and follows Settings from now on.
pub fn init(cx: &mut App) {
    let (sender, mut receiver) = unbounded();
    let commands = cx.spawn(async move |cx| {
        while let Some(command) = receiver.next().await {
            cx.update(|cx| run(command, cx));
        }
    });
    cx.set_global(SystemTray { sender, icon: None, language: None, _commands: commands });
    sync(cx);
    cx.observe_global::<settings::SettingsStore>(sync).detach();
}

/// Whether closing the main window hides it into the tray: the icon shows and Settings asks
/// for it.
pub fn keeps_playing_when_closed(cx: &App) -> bool {
    let shown = cx.try_global::<SystemTray>().is_some_and(|tray| tray.icon.is_some());
    shown && settings::get(cx).prefs.interface.keep_playing_in_tray
}

/// Shows the main window if the tray hid it; `false` when there's no hidden one.
pub fn show_hidden_main_window(cx: &mut App) -> bool {
    let Some(main) = main_window(cx) else { return false };
    let hidden = main.read(cx).is_ok_and(MainWindow::is_hidden_in_tray);
    if hidden {
        main.update(cx, MainWindow::show_from_tray).ok();
    }
    hidden
}

/// Shows or takes away the icon to match the setting, and makes it again when the interface
/// language changes so its menu (built once on macOS) speaks the new one.
fn sync(cx: &mut App) {
    let prefs = &settings::get(cx).prefs;
    let wanted = prefs.interface.system_tray && crate::tray::is_available();
    let language = Language::resolve(prefs.language);
    let tray = cx.global_mut::<SystemTray>();
    if wanted && tray.language != Some(language) {
        tray.icon = None;
        tray.language = Some(language);
    }
    let tray = cx.global::<SystemTray>();
    if wanted == tray.icon.is_some() {
        return;
    }
    let icon = wanted.then(|| TrayIcon::show(tray.sender.clone())).and_then(|shown| {
        shown.inspect_err(|err| tracing::warn!("couldn't show the tray icon: {err:#}")).ok()
    });
    let hidden_main =
        main_window(cx).filter(|main| main.read(cx).is_ok_and(MainWindow::is_hidden_in_tray));
    cx.global_mut::<SystemTray>().icon = icon;
    // Without the icon, a window hidden into it could never come back.
    if !wanted && let Some(main) = hidden_main {
        main.update(cx, MainWindow::show_from_tray).ok();
    }
}

fn run(command: TrayCommand, cx: &mut App) {
    if command == TrayCommand::Quit {
        crate::main_window::ask_before_quitting(cx);
        return;
    }
    if let Some(main) = main_window(cx) {
        main.update(cx, |main, window, cx| main.run_tray_command(command, window, cx)).ok();
    } else if command == TrayCommand::ShowVlc {
        if cx.windows().is_empty() {
            super::open_initial_window(&Launch::default(), cx);
        }
        cx.activate(true);
    }
}

fn main_window(cx: &App) -> Option<gpui::WindowHandle<MainWindow>> {
    cx.windows().into_iter().find_map(|window| window.downcast::<MainWindow>())
}
