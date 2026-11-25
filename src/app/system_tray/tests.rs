//! The tray icon follows Settings, and its menu's commands reach the main window or the app,
//! with a stand-in tray in place of the OS's.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use gpui::{Empty, TestAppContext, VisualTestContext};

use super::*;
use crate::app::media::Media;
use crate::convert::{self, Conversions, Destination, Preset};
use crate::main_window::tests::{has_dialog, hide_in_tray, main_view, open};
use crate::settings::InterfaceLanguage;

/// What the desktop does with an icon.
#[derive(Clone, Copy, Default)]
enum Desktop {
    /// It shows it.
    #[default]
    Shows,
    /// It has no tray.
    NoTray,
    /// It has a tray that won't take the icon.
    Refuses,
}

/// A tray that counts the icons it was asked for and those showing, and keeps the sender the
/// last one was given.
#[derive(Clone, Default)]
struct StandIn {
    desktop: Desktop,
    made: Rc<Cell<usize>>,
    showing: Rc<Cell<usize>>,
    sender: Rc<RefCell<Option<CommandSender>>>,
}

/// An icon in the stand-in tray, gone when dropped.
struct StandInIcon(Rc<Cell<usize>>);

impl Drop for StandInIcon {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}

impl Tray for StandIn {
    fn is_available(&self) -> bool {
        !matches!(self.desktop, Desktop::NoTray)
    }

    fn show(&self, sender: CommandSender) -> anyhow::Result<Box<dyn Any>> {
        self.made.set(self.made.get() + 1);
        if matches!(self.desktop, Desktop::Refuses) {
            anyhow::bail!("the tray refused the icon");
        }
        self.sender.replace(Some(sender));
        self.showing.set(self.showing.get() + 1);
        Ok(Box::new(StandInIcon(self.showing.clone())))
    }
}

impl StandIn {
    /// Sends `command` as the icon's menu does, and lets the app run it.
    fn choose(&self, command: TrayCommand, cx: &mut TestAppContext) {
        let sender = self.sender.borrow().clone().expect("an icon showed");
        sender.unbounded_send(command).unwrap();
        cx.run_until_parked();
    }
}

/// The app with Settings in memory, no window and `tray` as the OS's tray.
fn app_with(tray: &StandIn, cx: &mut TestAppContext) {
    cx.update(|cx| {
        settings::init_in_memory(cx);
        crate::theme::init(cx);
        crate::ui::init(cx);
        init_with_tray(tray.clone(), cx);
    });
}

/// The main window, with `tray` as the OS's tray.
fn main_with<'a>(tray: &StandIn, cx: &'a mut TestAppContext) -> &'a mut VisualTestContext {
    let cx = open(cx);
    cx.update(|_, cx| init_with_tray(tray.clone(), cx));
    cx
}

fn change(cx: &mut TestAppContext, f: impl FnOnce(&mut settings::Settings)) {
    cx.update(|cx| settings::update(cx, f));
    cx.run_until_parked();
}

fn show_icon(cx: &mut TestAppContext) {
    change(cx, |s| s.prefs.interface.system_tray = true);
}

fn keeps_playing(cx: &mut TestAppContext) -> bool {
    cx.update(|cx| keeps_playing_when_closed(cx))
}

fn is_hidden(cx: &mut VisualTestContext) -> bool {
    let main = main_view(cx);
    main.read_with(cx, |main, _| main.is_hidden_in_tray())
}

#[gpui::test]
fn the_icon_shows_while_settings_ask_for_it(cx: &mut TestAppContext) {
    let tray = StandIn::default();
    app_with(&tray, cx);
    assert_eq!(tray.made.get(), 0, "off by default");
    show_icon(cx);
    assert_eq!(tray.showing.get(), 1);
    change(cx, |s| s.prefs.interface.system_tray = false);
    assert_eq!(tray.showing.get(), 0);
}

#[gpui::test]
fn closing_keeps_playing_only_with_the_icon_and_the_setting(cx: &mut TestAppContext) {
    let tray = StandIn::default();
    app_with(&tray, cx);
    change(cx, |s| s.prefs.interface.keep_playing_in_tray = true);
    assert!(!keeps_playing(cx), "no icon to come back from");
    show_icon(cx);
    assert!(keeps_playing(cx));
    change(cx, |s| s.prefs.interface.keep_playing_in_tray = false);
    assert!(!keeps_playing(cx));
}

#[gpui::test]
fn without_the_tray_closing_closes(cx: &mut TestAppContext) {
    cx.update(settings::init_in_memory);
    assert!(!keeps_playing(cx));
}

#[gpui::test]
fn a_desktop_without_a_tray_gets_no_icon(cx: &mut TestAppContext) {
    let tray = StandIn { desktop: Desktop::NoTray, ..StandIn::default() };
    app_with(&tray, cx);
    show_icon(cx);
    assert_eq!(tray.made.get(), 0);
}

#[gpui::test]
fn an_icon_the_tray_refuses_leaves_none(cx: &mut TestAppContext) {
    let tray = StandIn { desktop: Desktop::Refuses, ..StandIn::default() };
    app_with(&tray, cx);
    change(cx, |s| s.prefs.interface.keep_playing_in_tray = true);
    show_icon(cx);
    assert_eq!(tray.made.get(), 1);
    assert!(!keeps_playing(cx), "closing must close: nothing could bring the window back");
}

#[gpui::test]
fn a_new_language_makes_the_icon_again(cx: &mut TestAppContext) {
    let tray = StandIn::default();
    app_with(&tray, cx);
    show_icon(cx);
    change(cx, |s| s.prefs.language = InterfaceLanguage::PseudoAccented);
    assert_eq!((tray.made.get(), tray.showing.get()), (2, 1));
    change(cx, |s| s.prefs.interface.keep_playing_in_tray = true);
    assert_eq!(tray.made.get(), 2, "other settings leave it be");
}

#[gpui::test]
fn taking_the_icon_away_brings_back_a_hidden_window(cx: &mut TestAppContext) {
    let tray = StandIn::default();
    let cx = main_with(&tray, cx);
    show_icon(cx);
    hide_in_tray(cx);
    change(cx, |s| s.prefs.interface.system_tray = false);
    assert!(!is_hidden(cx));
}

#[gpui::test]
fn only_a_hidden_main_window_is_shown_again(cx: &mut TestAppContext) {
    cx.update(settings::init_in_memory);
    assert!(!cx.update(show_hidden_main_window), "no window");
    let tray = StandIn::default();
    let cx = main_with(&tray, cx);
    // Run from the app, as reopening does, not from inside the window.
    let app: &mut TestAppContext = cx;
    assert!(!app.update(show_hidden_main_window), "it shows already");
    hide_in_tray(cx);
    let app: &mut TestAppContext = cx;
    assert!(app.update(show_hidden_main_window));
    assert!(!is_hidden(cx));
}

#[gpui::test]
fn show_vlc_brings_back_the_main_window(cx: &mut TestAppContext) {
    let tray = StandIn::default();
    let cx = main_with(&tray, cx);
    show_icon(cx);
    hide_in_tray(cx);
    tray.choose(TrayCommand::ShowVlc, cx);
    assert!(!is_hidden(cx));
}

#[gpui::test]
fn show_vlc_without_a_window_opens_one(cx: &mut TestAppContext) {
    let tray = StandIn::default();
    app_with(&tray, cx);
    show_icon(cx);
    tray.choose(TrayCommand::ShowVlc, cx);
    assert_eq!(cx.windows().len(), 1);
}

#[gpui::test]
fn show_vlc_beside_another_window_opens_no_more(cx: &mut TestAppContext) {
    let tray = StandIn::default();
    app_with(&tray, cx);
    show_icon(cx);
    cx.add_window(|_, _| Empty);
    tray.choose(TrayCommand::ShowVlc, cx);
    assert_eq!(cx.windows().len(), 1);
}

#[gpui::test]
fn playback_commands_need_the_main_window(cx: &mut TestAppContext) {
    let tray = StandIn::default();
    app_with(&tray, cx);
    show_icon(cx);
    for command in [TrayCommand::PlayPause, TrayCommand::Previous, TrayCommand::Next] {
        tray.choose(command, cx);
    }
    assert_eq!(cx.windows().len(), 0);
}

#[gpui::test]
fn quit_asks_about_running_conversions_first(cx: &mut TestAppContext) {
    let tray = StandIn::default();
    let cx = main_with(&tray, cx);
    show_icon(cx);
    cx.update(|_, cx| {
        convert::init(cx);
        let conversions = Conversions::global(cx).expect("started");
        conversions.update(cx, |conversions, _| {
            let source = Media::File(PathBuf::from("/videos/a.mkv"));
            let queue = conversions.queue_mut();
            queue.add(source, Preset::default(), &Destination::NextToSource, |_| false);
        });
    });
    tray.choose(TrayCommand::Quit, cx);
    assert!(has_dialog(cx));
}
