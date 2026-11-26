//! The Bookmarks tab on a window playing on the mock backend, its bookmarks kept in an
//! in-memory database: adding, going to, renaming and removing them, by mouse and keyboard.

use std::time::Duration;

use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext};

use super::super::ListTab;
use super::store::Bookmarkable;
use crate::engine::backend::BackendEvent;
use crate::engine::mock::{MockHandle, MockMedia};
use crate::engine::values::SeekTarget;
use crate::history::tests::support::TestHistory;
use crate::main_window::MainWindow;
use crate::main_window::tests::{main_view, open, tap};
use crate::main_window::video_menu::tests::support::{emit, player, playing};

/// Cmd+B on macOS, Ctrl+B elsewhere.
const ADD: &str = if cfg!(target_os = "macos") { "cmd-b" } else { "ctrl-b" };

/// The window playing `media` with its bookmarks read, the Bookmarks tab open and drawn.
fn bookmarks_tab(
    cx: &mut TestAppContext,
    media: MockMedia,
) -> (&mut VisualTestContext, TestHistory, MockHandle) {
    let mut history = TestHistory::install(cx);
    let (cx, mock) = playing(cx, media);
    history.settle(cx);
    show_tab(cx);
    (cx, history, mock)
}

fn show_tab(cx: &mut VisualTestContext) {
    let main = main_view(cx);
    main.update_in(cx, |main, window, cx| main.toggle_list_tab(ListTab::Bookmarks, window, cx));
    draw(cx);
}

fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
}

/// Plays from `seconds` on.
fn seek(cx: &mut VisualTestContext, seconds: u64) {
    let player = player(cx);
    let to = SeekTarget::Absolute(Duration::from_secs(seconds));
    player.update(cx, |player, cx| player.seek(to, cx));
    cx.run_until_parked();
}

/// Bookmarks where playback is, at `seconds`.
fn add_at(cx: &mut VisualTestContext, seconds: u64) {
    seek(cx, seconds);
    tap(cx, ADD);
    draw(cx);
}

fn names(cx: &mut VisualTestContext) -> Vec<String> {
    let main = main_view(cx);
    main.read_with(cx, |main, cx| main.bookmark_list(cx).iter().map(|b| b.name.clone()).collect())
}

fn bookmarkable(cx: &mut VisualTestContext) -> Bookmarkable {
    let main = main_view(cx);
    main.read_with(cx, MainWindow::bookmarkable)
}

fn position(cx: &mut VisualTestContext) -> Duration {
    let player = player(cx);
    player.read_with(cx, |player, _| player.position())
}

fn click(cx: &mut VisualTestContext, id: &'static str, clicks: usize) {
    let at = cx.debug_bounds(id).unwrap_or_else(|| panic!("{id} isn't drawn")).center();
    let modifiers = Modifiers::none();
    cx.simulate_event(gpui::MouseDownEvent {
        position: at,
        button: MouseButton::Left,
        modifiers,
        click_count: clicks,
        first_mouse: false,
    });
    cx.simulate_event(gpui::MouseUpEvent {
        position: at,
        button: MouseButton::Left,
        modifiers,
        click_count: clicks,
    });
    draw(cx);
}

fn renaming(cx: &mut VisualTestContext) -> bool {
    let main = main_view(cx);
    main.read_with(cx, |main, _| main.queue_panel.is_renaming_bookmark())
}

#[gpui::test]
fn nothing_loaded_takes_no_bookmarks(cx: &mut TestAppContext) {
    let cx = open(cx);
    show_tab(cx);
    assert_eq!(bookmarkable(cx), Bookmarkable::NothingLoaded);
    assert!(cx.debug_bounds("bookmark-row-0").is_none());
}

#[gpui::test]
fn a_live_stream_takes_no_bookmarks(cx: &mut TestAppContext) {
    let live = MockMedia { duration: None, seekable: false, ..MockMedia::default() };
    let (cx, _history, _mock) = bookmarks_tab(cx, live);
    assert_eq!(bookmarkable(cx), Bookmarkable::CantSeek);
    add_at(cx, 5);
    assert_eq!(names(cx), Vec::<String>::new());
}

#[gpui::test]
fn bookmarks_are_added_in_order_and_a_click_goes_there(cx: &mut TestAppContext) {
    let (cx, mut history, _mock) = bookmarks_tab(cx, MockMedia::default());
    assert_eq!(bookmarkable(cx), Bookmarkable::Yes);
    assert!(cx.debug_bounds("bookmark-row-0").is_none(), "no bookmarks yet");
    add_at(cx, 30);
    add_at(cx, 10);
    add_at(cx, 10);
    assert_eq!(names(cx), ["Clip 2", "Clip 1"], "by place, one per place");
    history.settle(cx);
    let stored = history.read(|connection| {
        connection.query_row("SELECT COUNT(*) FROM bookmarks", [], |row| row.get::<_, i64>(0))
    });
    assert_eq!(stored.unwrap(), 2);

    click(cx, "bookmark-row-1", 1);
    assert_eq!(position(cx), Duration::from_secs(30));
}

#[gpui::test]
fn the_keyboard_moves_goes_and_removes(cx: &mut TestAppContext) {
    let (cx, _history, _mock) = bookmarks_tab(cx, MockMedia::default());
    for seconds in [10, 20, 30] {
        add_at(cx, seconds);
    }
    seek(cx, 0);
    tap(cx, "home");
    tap(cx, "down");
    tap(cx, "enter");
    assert_eq!(position(cx), Duration::from_secs(20));
    tap(cx, "end");
    tap(cx, "up");
    tap(cx, "delete");
    assert_eq!(names(cx), ["Clip 1", "Clip 3"]);
    tap(cx, "backspace");
    assert_eq!(names(cx), ["Clip 1"], "the cursor stays on the last row");
}

#[gpui::test]
fn f2_and_double_click_rename_and_escape_cancels(cx: &mut TestAppContext) {
    let (cx, _history, _mock) = bookmarks_tab(cx, MockMedia::default());
    add_at(cx, 10);
    tap(cx, "f2");
    assert!(renaming(cx));
    cx.simulate_input("Intro");
    tap(cx, "enter");
    assert!(!renaming(cx));
    assert_eq!(names(cx), ["Intro"]);

    click(cx, "bookmark-row-0", 2);
    assert!(renaming(cx));
    tap(cx, "escape");
    assert!(!renaming(cx));
    assert_eq!(names(cx), ["Intro"]);
}

#[gpui::test]
fn a_bookmark_past_the_end_stays_but_cant_be_gone_to(cx: &mut TestAppContext) {
    let long = MockMedia { duration: Some(Duration::from_secs(600)), ..MockMedia::default() };
    let (cx, _history, mock) = bookmarks_tab(cx, long);
    add_at(cx, 300);
    seek(cx, 5);
    emit(cx, &mock, BackendEvent::Duration(Some(Duration::from_secs(120))));
    draw(cx);
    click(cx, "bookmark-row-0", 1);
    assert_eq!(position(cx), Duration::from_secs(5));
    assert_eq!(names(cx), ["Clip 1"]);
}
