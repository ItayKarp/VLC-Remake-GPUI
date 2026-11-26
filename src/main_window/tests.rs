//! Headless tests of the main window with the real keymap.

use gpui::{TestAppContext, VisualTestContext};

use super::MainWindow;
use super::tray::InTray;
use crate::app::request::OpenRequest;
use crate::keymap;
use crate::ui::menu::MenuItem;
use crate::ui::toast::Level;
use crate::ui::toast::tests::Shown;

/// The main window with the real keymap, its player keys focused.
pub(crate) fn open(cx: &mut TestAppContext) -> &mut VisualTestContext {
    cx.update(|cx| {
        crate::settings::init_in_memory(cx);
        crate::theme::init(cx);
        crate::ui::init(cx);
        keymap::init(cx);
    });
    let (main, cx) =
        cx.add_window_view(|window, cx| MainWindow::new(OpenRequest::default(), window, cx));
    main.update_in(cx, |main, window, cx| main.focus_handle.focus(window, cx));
    cx.run_until_parked();
    cx
}

/// The window's view.
pub(crate) fn main_view(cx: &mut VisualTestContext) -> gpui::Entity<MainWindow> {
    cx.update(|window, _| window.root::<MainWindow>().flatten().expect("the main window's view"))
}

/// Hides the window into the tray, as closing it does where the OS can hide windows.
pub(crate) fn hide_in_tray(cx: &mut VisualTestContext) {
    let main = main_view(cx);
    main.update(cx, |main, _| main.windowing.tray = InTray::Hidden);
}

/// Whether the window shows a dialog.
pub(crate) fn has_dialog(cx: &mut VisualTestContext) -> bool {
    let main = main_view(cx);
    main.read_with(cx, |main, cx| main.modals.read(cx).is_open())
}

fn is_fullscreen(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, _| window.is_fullscreen())
}

/// The window's toasts, oldest first: each message and its buttons' labels.
pub(super) fn toasts(cx: &mut VisualTestContext) -> Vec<(String, Vec<String>)> {
    shown(cx).into_iter().map(|toast| (toast.message, toast.buttons)).collect()
}

/// The messages of the window's toasts, oldest first.
pub(super) fn toast_messages(cx: &mut VisualTestContext) -> Vec<String> {
    shown(cx).into_iter().map(|toast| toast.message).collect()
}

/// The window's toasts, oldest first: each level and message.
pub(super) fn toast_levels(cx: &mut VisualTestContext) -> Vec<(Level, String)> {
    shown(cx).into_iter().map(|toast| (toast.level, toast.message)).collect()
}

fn shown(cx: &mut VisualTestContext) -> Vec<Shown> {
    let main = main_view(cx);
    main.read_with(cx, |main, cx| main.toasts.read(cx).shown())
}

/// Presses the newest toast's button labelled `label`, as a click does, and settles the app.
pub(super) fn press_toast(cx: &mut VisualTestContext, label: &str) {
    let main = main_view(cx);
    let toasts = main.read_with(cx, |main, _| main.toasts.clone());
    cx.update(|window, cx| toasts.update(cx, |toasts, cx| toasts.press_button(label, window, cx)));
    cx.run_until_parked();
}

/// Presses `key` and releases it, as a person does.
pub(super) fn tap(cx: &mut VisualTestContext, key: &'static str) {
    cx.simulate_keystrokes(key);
    cx.simulate_event(gpui::KeyUpEvent { keystroke: gpui::Keystroke::parse(key).unwrap() });
    cx.run_until_parked();
}

#[gpui::test]
fn f_toggles_fullscreen_and_escape_leaves_it(cx: &mut TestAppContext) {
    let cx = open(cx);
    tap(cx, "f");
    assert!(is_fullscreen(cx));
    tap(cx, "f");
    assert!(!is_fullscreen(cx));
    tap(cx, "f");
    tap(cx, "escape");
    assert!(!is_fullscreen(cx));
}

#[gpui::test]
fn escape_in_a_window_never_enters_fullscreen(cx: &mut TestAppContext) {
    let cx = open(cx);
    tap(cx, "escape");
    tap(cx, "escape");
    assert!(!is_fullscreen(cx));
}

#[gpui::test]
fn a_held_escape_leaves_fullscreen_once(cx: &mut TestAppContext) {
    let cx = open(cx);
    tap(cx, "f");
    // The OS repeats a held key without a key-up in between.
    cx.simulate_keystrokes("escape escape escape");
    cx.run_until_parked();
    assert!(!is_fullscreen(cx));
}

#[gpui::test]
fn the_main_menu_lists_the_shipped_commands(cx: &mut TestAppContext) {
    let cx = open(cx);
    let main = main_view(cx);
    let labels: Vec<String> = main.update_in(cx, |_, window, cx| {
        let items = super::main_menu::items(&cx.entity().downgrade(), window, cx);
        items
            .iter()
            .filter_map(|item| match item {
                MenuItem::Action { label, .. } | MenuItem::Submenu { label, .. } => {
                    Some(label.to_string())
                }
                _ => None,
            })
            .collect()
    });
    let shown = ["Open File(s)", "Open Folder", "Open Recent", "Settings", "About VLC"];
    let shipped = ["Customize Player", "Open Disc", "Open Capture Device", "Convert"];
    for shown in shown.into_iter().chain(shipped) {
        assert!(labels.iter().any(|label| label == shown), "{shown} missing: {labels:?}");
    }
}

#[gpui::test]
fn shift_f1_opens_about(cx: &mut TestAppContext) {
    let cx = open(cx);
    tap(cx, "shift-f1");
    let main = main_view(cx);
    assert!(main.read_with(cx, |main, cx| main.modals.read(cx).is_open()));
}
