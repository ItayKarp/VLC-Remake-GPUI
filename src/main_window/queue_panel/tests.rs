//! Headless tests of the queue panel with the real keymap.

use std::path::PathBuf;

use gpui::{TestAppContext, VisualTestContext};

use crate::app::media::Media;
use crate::main_window::tests::{main_view, open, tap};

/// Opens the window with `count` queued files.
fn with_queue(cx: &mut TestAppContext, count: usize) -> &mut VisualTestContext {
    let cx = open(cx);
    let main = main_view(cx);
    main.update(cx, |main, _| {
        main.queue
            .replace((0..count).map(|i| Media::File(PathBuf::from(format!("{i}.mkv")))).collect());
    });
    cx
}

fn is_open(cx: &mut VisualTestContext) -> bool {
    let main = main_view(cx);
    main.read_with(cx, |main, _| main.queue_panel.is_open())
}

fn queued(cx: &mut VisualTestContext) -> Vec<String> {
    let main = main_view(cx);
    main.read_with(cx, |main, _| {
        main.queue.entries().iter().map(|entry| entry.media.display_name()).collect()
    })
}

#[gpui::test]
fn q_and_escape_open_and_close_the_panel(cx: &mut TestAppContext) {
    let cx = with_queue(cx, 2);
    tap(cx, "q");
    assert!(is_open(cx));
    tap(cx, "q");
    assert!(!is_open(cx));
    tap(cx, "q");
    tap(cx, "escape");
    assert!(!is_open(cx));
}

#[gpui::test]
fn the_keyboard_selects_and_deletes_rows(cx: &mut TestAppContext) {
    let cx = with_queue(cx, 4);
    tap(cx, "q");
    tap(cx, "down");
    tap(cx, "shift-down");
    tap(cx, "delete");
    assert_eq!(queued(cx), ["2.mkv", "3.mkv"]);
    // The row that took their place is selected, so Delete carries on.
    tap(cx, "backspace");
    assert_eq!(queued(cx), ["3.mkv"]);
    assert!(is_open(cx));
}

#[gpui::test]
fn selecting_every_row_then_deleting_empties_the_queue(cx: &mut TestAppContext) {
    let cx = with_queue(cx, 3);
    tap(cx, "q");
    tap(cx, "home");
    tap(cx, "shift-end");
    tap(cx, "delete");
    assert_eq!(queued(cx), Vec::<String>::new());
}
