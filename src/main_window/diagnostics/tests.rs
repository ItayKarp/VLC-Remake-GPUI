//! The crash toast and Copy Debug Info, from the main window.

use gpui::TestAppContext;

use crate::main_window::MainWindow;
use crate::main_window::tests::{main_view, open};
use crate::main_window::video_menu::tests::support::{press_toast, toast_messages, toasts};
use crate::opening::scratch::scratch_dir;

const REPORT: &str = "thread 'main' panicked at src/main.rs:1:1:\nboom\n";

#[gpui::test]
fn a_saved_crash_report_can_be_filed_on_github(cx: &mut TestAppContext) {
    let cx = open(cx);
    let report = scratch_dir().join("crash.txt");
    std::fs::write(&report, REPORT).unwrap();
    let main = main_view(cx);
    main.update(cx, |main, cx| main.tell_crash(report, cx));
    let (message, buttons) = toasts(cx).pop().expect("the crash toast");
    assert!(message.starts_with("VLC closed unexpectedly last time."), "{message}");
    assert_eq!(buttons, ["Show Report", "Report on GitHub"]);

    press_toast(cx, "Report on GitHub");
    let opened = cx.opened_url().expect("a new issue opens");
    assert!(opened.starts_with("https://github.com/"), "{opened}");
}

#[gpui::test]
fn a_report_gone_since_opens_nothing(cx: &mut TestAppContext) {
    let cx = open(cx);
    let main = main_view(cx);
    main.update(cx, |main, cx| main.tell_crash(scratch_dir().join("gone.txt"), cx));
    press_toast(cx, "Report on GitHub");
    assert_eq!(cx.opened_url(), None);
}

#[gpui::test]
fn copy_debug_info_fills_the_clipboard(cx: &mut TestAppContext) {
    let cx = open(cx);
    let main = main_view(cx);
    main.update_in(cx, MainWindow::copy_debug_info);
    let copied = cx.read_from_clipboard().and_then(|item| item.text()).unwrap_or_default();
    assert_ne!(copied, "");
    assert_eq!(toast_messages(cx), ["Debug info copied to the clipboard."]);
}
