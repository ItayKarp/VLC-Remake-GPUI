//! Headless tests of the listing's item menus (S4.8): which menu an item has, what several
//! selected items disable, and opening one from the keyboard.

use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext};

use super::{browse, selected};
use crate::main_window::tests::{main_view, open, tap};
use crate::navigation::route::Route;
use crate::opening::scratch::scratch_dir;

/// The open item menu's entries: each label, and whether it can be chosen.
fn entries(cx: &mut VisualTestContext) -> Vec<(String, bool)> {
    let main = main_view(cx);
    cx.update(|_, cx| {
        let main = main.read(cx);
        let (menu, _, _) = main.library_panel.item_menu.as_ref().expect("a menu is open");
        menu.read(cx).entries()
    })
}

/// The open item menu's labels.
fn labels(cx: &mut VisualTestContext) -> Vec<String> {
    entries(cx).into_iter().map(|(label, _)| label).collect()
}

/// Right-clicks the element drawn with debug id `id`.
fn right_click(cx: &mut VisualTestContext, id: &'static str) {
    let at = cx.debug_bounds(id).expect("the item is drawn").center();
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
}

/// Whether the open item menu's `label` can be chosen.
fn enabled(cx: &mut VisualTestContext, label: &str) -> bool {
    let entries = entries(cx);
    entries.iter().find(|(shown, _)| shown == label).expect("the menu has it").1
}

#[gpui::test]
fn right_clicking_a_card_selects_it_and_opens_the_file_menu(cx: &mut TestAppContext) {
    let cx = browse(cx, 3);
    right_click(cx, "library-card-1");
    assert_eq!(selected(cx), ["Show S01E02"]);
    let labels = labels(cx);
    let expected = [
        "Play",
        "Mark as Watched",
        "Favorite",
        "Stream",
        "Convert",
        "Add to Queue",
        "Add to Playlist",
    ];
    assert_eq!(labels[..expected.len()], expected);
    let tail = ["Go to Source", "Add Subtitle Tracks", "Add Audio Tracks", "Properties & Metadata"];
    assert_eq!(labels[labels.len() - tail.len()..], tail);
    assert!(enabled(cx, "Go to Source"));
}

#[gpui::test]
fn several_cards_disable_what_works_on_one(cx: &mut TestAppContext) {
    let cx = browse(cx, 3);
    tap(cx, if cfg!(target_os = "macos") { "cmd-a" } else { "ctrl-a" });
    right_click(cx, "library-card-0");
    assert_eq!(selected(cx).len(), 3, "the selection stays");
    assert!(enabled(cx, "Play"));
    assert!(enabled(cx, "Convert"), "every card picked converts");
    assert!(enabled(cx, "Add to Queue"));
    for one_only in
        ["Go to Source", "Add Subtitle Tracks", "Add Audio Tracks", "Properties & Metadata"]
    {
        assert!(!enabled(cx, one_only), "{one_only} is enabled");
    }
}

#[gpui::test]
fn shift_f10_opens_the_keyboard_items_menu(cx: &mut TestAppContext) {
    let cx = browse(cx, 2);
    tap(cx, "down");
    tap(cx, "down");
    tap(cx, "shift-f10");
    assert_eq!(selected(cx), ["Show S01E02"]);
    assert_eq!(labels(cx)[0], "Play");
}

#[gpui::test]
fn add_to_queue_queues_every_selected_card(cx: &mut TestAppContext) {
    let cx = browse(cx, 3);
    tap(cx, if cfg!(target_os = "macos") { "cmd-a" } else { "ctrl-a" });
    tap(cx, "shift-f10");
    // Typing jumps to the next item starting with the letter: Add to Queue.
    cx.simulate_input("a");
    tap(cx, "enter");
    cx.run_until_parked();
    let main = main_view(cx);
    let queued = main.read_with(cx, |main, _| {
        main.queue.entries().iter().map(|entry| entry.media.display_name()).collect::<Vec<_>>()
    });
    assert_eq!(queued, ["Show S01E01.mkv", "Show S01E02.mkv", "Show S01E03.mkv"]);
}

#[gpui::test]
fn a_folder_row_has_the_folder_menu(cx: &mut TestAppContext) {
    let dir = scratch_dir();
    std::fs::create_dir(dir.join("Season 1")).expect("subfolder");
    std::fs::write(dir.join("Pilot.mkv"), b"x").expect("file");
    let cx = open(cx);
    let main = main_view(cx);
    main.update_in(cx, |main, window, cx| {
        main.navigate(Route::Videos(Some(dir)), cx);
        main.toggle_library_panel(window, cx);
    });
    cx.run_until_parked();
    // Row 0 is "+ Add", which has no menu.
    right_click(cx, "library-row-0");
    assert!(main.read_with(cx, |main, _| main.library_panel.item_menu.is_none()));
    right_click(cx, "library-row-1");
    let expected = ["Add File", "Add Folder", "Edit Subtitle Tracks", "Resync", "Info"];
    assert_eq!(labels(cx), expected);
}
