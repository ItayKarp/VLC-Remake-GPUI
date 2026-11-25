//! The Chapters tab on a window playing on the mock backend: its rows go to their chapter, and
//! the keyboard moves through them.

use std::time::Duration;

use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext};

use super::super::ListTab;
use crate::engine::mock::{MockHandle, MockMedia};
use crate::main_window::tests::{main_view, tap};
use crate::main_window::video_menu::tests::support::{chapters, emit, player, playing};

/// The window playing ten minutes with `titles` a minute apart, the Chapters tab open.
fn chapters_tab<'a>(
    cx: &'a mut TestAppContext,
    titles: &[&str],
) -> (&'a mut VisualTestContext, MockHandle) {
    let media = MockMedia { duration: Some(Duration::from_secs(600)), ..MockMedia::default() };
    let (cx, mock) = playing(cx, media);
    emit(cx, &mock, chapters(titles));
    let main = main_view(cx);
    main.update_in(cx, |main, window, cx| main.toggle_list_tab(ListTab::Chapters, window, cx));
    draw(cx);
    (cx, mock)
}

fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
}

fn position(cx: &mut VisualTestContext) -> Duration {
    let player = player(cx);
    player.read_with(cx, |player, _| player.position())
}

fn minutes(count: u64) -> Duration {
    Duration::from_secs(60 * count)
}

#[gpui::test]
fn media_without_chapters_shows_no_rows(cx: &mut TestAppContext) {
    let (cx, _mock) = chapters_tab(cx, &[]);
    assert!(cx.debug_bounds("chapter-row-0").is_none());
    tap(cx, "down");
    assert_eq!(position(cx), Duration::ZERO);
}

#[gpui::test]
fn clicking_a_row_goes_to_its_chapter(cx: &mut TestAppContext) {
    let (cx, _mock) = chapters_tab(cx, &["Opening", "Middle", "Credits"]);
    let at = cx.debug_bounds("chapter-row-2").expect("the third row is drawn").center();
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    draw(cx);
    assert_eq!(position(cx), minutes(2));
}

#[gpui::test]
fn the_keyboard_moves_the_cursor_and_enter_goes(cx: &mut TestAppContext) {
    let (cx, _mock) = chapters_tab(cx, &["Opening", "Middle", "Credits"]);
    tap(cx, "enter");
    assert_eq!(position(cx), Duration::ZERO, "no cursor yet");
    tap(cx, "down");
    tap(cx, "down");
    tap(cx, "enter");
    assert_eq!(position(cx), minutes(1));
    tap(cx, "end");
    tap(cx, "enter");
    assert_eq!(position(cx), minutes(2));
    tap(cx, "home");
    tap(cx, "up");
    tap(cx, "enter");
    assert_eq!(position(cx), Duration::ZERO);
    draw(cx);
}
