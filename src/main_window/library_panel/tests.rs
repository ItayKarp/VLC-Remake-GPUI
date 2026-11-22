//! Headless tests of browsing a folder in the sidebar, as a list and as a grid, with the real
//! keymap.

use std::path::PathBuf;

use gpui::{Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext, point, px};

use super::MainWindow;
use super::listing::Body;
use crate::main_window::tests::{main_view, open, tap};
use crate::navigation::route::{MusicRoute, Route};
use crate::navigation::view::View;
use crate::opening::scratch::scratch_dir;

/// The main window with the sidebar open on a folder of `count` episodes.
fn browse(cx: &mut TestAppContext, count: usize) -> &mut VisualTestContext {
    let dir = scratch_dir();
    for episode in 1..=count {
        std::fs::write(dir.join(format!("Show S01E{episode:02}.mkv")), b"x").expect("file");
    }
    let cx = open(cx);
    let main = main_view(cx);
    main.update_in(cx, |main, window, cx| {
        main.navigate(Route::Videos(Some(dir)), cx);
        main.toggle_library_panel(window, cx);
    });
    cx.run_until_parked();
    cx
}

/// The names of the selected cards, in list order.
fn selected(cx: &mut VisualTestContext) -> Vec<String> {
    let main = main_view(cx);
    cx.update(|_, cx| {
        let main = main.read(cx);
        let Body::Items(items) = main.listing(cx) else { panic!("the folder was read") };
        let keys = items.keys();
        let indices = main.library_panel.selection.indices(&keys);
        let name = |index: usize| {
            let card = &items.cards[index - items.rows.len()];
            PathBuf::from(&card.label).file_stem().unwrap().to_string_lossy().into_owned()
        };
        indices.into_iter().map(name).collect()
    })
}

#[gpui::test]
fn the_folder_lists_its_episodes(cx: &mut TestAppContext) {
    let cx = browse(cx, 3);
    let main = main_view(cx);
    let count = cx.update(|_, cx| main.read(cx).listed_count(cx));
    assert_eq!(count, 3);
}

#[gpui::test]
fn arrows_select_and_shift_arrows_extend(cx: &mut TestAppContext) {
    let cx = browse(cx, 5);
    tap(cx, "down");
    tap(cx, "down");
    assert_eq!(selected(cx), ["Show S01E02"]);
    tap(cx, "shift-down");
    tap(cx, "shift-down");
    assert_eq!(selected(cx), ["Show S01E02", "Show S01E03", "Show S01E04"]);
    tap(cx, "up");
    assert_eq!(selected(cx), ["Show S01E03"]);
    tap(cx, "end");
    assert_eq!(selected(cx), ["Show S01E05"]);
}

#[gpui::test]
fn alt_left_still_goes_back_from_the_listing(cx: &mut TestAppContext) {
    let cx = browse(cx, 2);
    tap(cx, "alt-left");
    let main = main_view(cx);
    let route = cx.update(|_, cx| main.read(cx).library_panel.history.current().clone());
    // The sidebar starts at Home, so Back leaves the folder for it.
    assert_eq!(route, Route::Home);
}

#[gpui::test]
fn select_all_takes_every_card(cx: &mut TestAppContext) {
    let cx = browse(cx, 4);
    tap(cx, if cfg!(target_os = "macos") { "cmd-a" } else { "ctrl-a" });
    assert_eq!(selected(cx).len(), 4);
}

/// Shows the current section as `view`, as the View menu does.
fn show_as(cx: &mut VisualTestContext, view: View) {
    let main = main_view(cx);
    main.update(cx, |main, cx| main.set_library_view(view, cx));
    cx.run_until_parked();
}

#[gpui::test]
fn in_the_grid_up_and_down_jump_a_line(cx: &mut TestAppContext) {
    let cx = browse(cx, 5);
    show_as(cx, View::Grid);
    tap(cx, "down");
    tap(cx, "down");
    assert_eq!(selected(cx), ["Show S01E03"]);
    tap(cx, "right");
    tap(cx, "shift-up");
    assert_eq!(selected(cx), ["Show S01E02", "Show S01E03", "Show S01E04"]);
}

#[gpui::test]
fn each_section_keeps_its_view(cx: &mut TestAppContext) {
    let cx = browse(cx, 1);
    show_as(cx, View::Grid);
    let main = main_view(cx);
    let view = |cx: &mut VisualTestContext| cx.update(|_, cx| main.read(cx).library_view(cx));
    assert_eq!(view(cx), View::Grid);
    main.update(cx, |main, cx| main.navigate(Route::Music(MusicRoute::Songs(None)), cx));
    assert_eq!(view(cx), View::List);
    main.update(cx, |main, cx| main.navigate(Route::Videos(None), cx));
    assert_eq!(view(cx), View::Grid);
}

/// Drags the first card onto the queue panel and returns what the queue holds then.
fn drag_first_card_to_the_queue(cx: &mut VisualTestContext) -> Vec<String> {
    let main = main_view(cx);
    main.update_in(cx, MainWindow::toggle_queue_panel);
    cx.run_until_parked();
    let card = cx.debug_bounds("library-card-0").expect("the first card is drawn").center();
    let queue = cx.debug_bounds("queue-rows-area").expect("the queue is open").center();
    let nudged: Point<Pixels> = point(card.x + px(10.), card.y + px(10.));
    let none = Modifiers::none();
    cx.simulate_mouse_down(card, MouseButton::Left, none);
    cx.simulate_mouse_move(nudged, MouseButton::Left, none);
    cx.simulate_mouse_move(queue, MouseButton::Left, none);
    cx.simulate_mouse_up(queue, MouseButton::Left, none);
    cx.run_until_parked();
    main.read_with(cx, |main, _| {
        main.queue.entries().iter().map(|entry| entry.media.display_name()).collect()
    })
}

#[gpui::test]
fn a_card_dragged_to_the_queue_is_queued(cx: &mut TestAppContext) {
    let cx = browse(cx, 3);
    assert_eq!(drag_first_card_to_the_queue(cx), ["Show S01E01.mkv"]);
}

#[gpui::test]
fn a_selected_card_drags_the_whole_selection(cx: &mut TestAppContext) {
    let cx = browse(cx, 3);
    tap(cx, if cfg!(target_os = "macos") { "cmd-a" } else { "ctrl-a" });
    let queued = drag_first_card_to_the_queue(cx);
    assert_eq!(queued, ["Show S01E01.mkv", "Show S01E02.mkv", "Show S01E03.mkv"]);
}

#[gpui::test]
fn a_drag_held_over_a_rail_icon_shows_its_section(cx: &mut TestAppContext) {
    let cx = browse(cx, 1);
    let card = cx.debug_bounds("library-card-0").expect("the card is drawn").center();
    let rail = cx.debug_bounds("rail-Playlists").expect("the rail is drawn").center();
    let none = Modifiers::none();
    cx.simulate_mouse_down(card, MouseButton::Left, none);
    cx.simulate_mouse_move(point(card.x + px(10.), card.y), MouseButton::Left, none);
    cx.simulate_mouse_move(rail, MouseButton::Left, none);
    let main = main_view(cx);
    let route = cx.update(|_, cx| main.read(cx).library_panel.history.current().clone());
    assert_eq!(route, Route::Playlists(None));
}
