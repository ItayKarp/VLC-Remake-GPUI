//! Headless tests of the main window with the real keymap.

use gpui::{TestAppContext, VisualTestContext};

use super::MainWindow;
use crate::app::request::OpenRequest;
use crate::keymap;

fn open(cx: &mut TestAppContext) -> &mut VisualTestContext {
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

fn is_fullscreen(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, _| window.is_fullscreen())
}

/// Presses `key` and releases it, as a person does.
fn tap(cx: &mut VisualTestContext, key: &'static str) {
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
