//! Profiles switched from the main window: the bars follow at once.

use gpui::TestAppContext;

use crate::main_window::tests::{main_view, open, tap};
use crate::main_window::video_menu::tests::support::{self, choose, find, toast_messages};
use crate::player_layout::Item;
use crate::settings::{self, StoredLayout, profiles};
use crate::ui::menu::MenuItem;

/// A one-row layout without the queue button.
fn without_queue() -> StoredLayout {
    let row = ["play-pause", "timeline"].map(str::to_owned).to_vec();
    StoredLayout { controls: vec![row], ..StoredLayout::default() }
}

#[gpui::test]
fn switching_profile_redraws_the_bars_with_its_layout(cx: &mut TestAppContext) {
    let cx = open(cx);
    cx.update(|_, cx| settings::update(cx, |s| s.prefs.player_layout = without_queue()));
    let main = main_view(cx);
    let has_queue = |cx: &mut gpui::VisualTestContext| {
        main.read_with(cx, |m, _| m.shown_layout().is_placed(Item::List))
    };
    cx.run_until_parked();
    assert!(!has_queue(cx));

    main.update(cx, |main, cx| main.choose_profile("Shows", cx));
    cx.run_until_parked();
    assert!(has_queue(cx), "Shows has the designed layout");

    main.update(cx, |main, cx| main.choose_profile("Movies", cx));
    cx.run_until_parked();
    assert!(!has_queue(cx), "Movies kept its own");
    assert_eq!(cx.update(|_, cx| profiles::active(cx)), "Movies");
}

#[gpui::test]
fn the_main_menu_names_the_profile_in_use(cx: &mut TestAppContext) {
    let cx = open(cx);
    let main = main_view(cx).downgrade();
    let (label, count) = cx.update(|_, cx| match super::profile_menu(&main, cx) {
        MenuItem::Submenu { label, items, .. } => (label, items.len()),
        _ => panic!("Profile is a submenu"),
    });
    // The four profiles, New, a separator, Rename, Delete and Use Automatically.
    assert_eq!(label.as_ref(), "Profile (Movies)");
    assert_eq!(count, 9);
}

/// Chooses the Profile menu's entry `label`, as it would open now.
fn choose_in_menu(cx: &mut gpui::VisualTestContext, label: &str) {
    let main = main_view(cx).downgrade();
    let menu = cx.update(|_, cx| super::profile_menu(&main, cx));
    choose(cx, find(std::slice::from_ref(&menu), &[menu_label(&menu), label]));
    draw(cx);
}

fn menu_label(menu: &MenuItem) -> &str {
    support::label(menu).expect("Profile has a label")
}

fn draw(cx: &mut gpui::VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
}

fn active(cx: &mut gpui::VisualTestContext) -> String {
    cx.update(|_, cx| profiles::active(cx))
}

fn names(cx: &mut gpui::VisualTestContext) -> Vec<String> {
    cx.update(|_, cx| profiles::list(cx).iter().map(|profile| profile.name.clone()).collect())
}

/// Types `name` in the open name dialog's field, which starts selected, and presses Enter.
fn answer(cx: &mut gpui::VisualTestContext, name: &str) {
    cx.simulate_input(name);
    tap(cx, "enter");
    draw(cx);
}

#[gpui::test]
fn choosing_a_profile_switches_to_it(cx: &mut TestAppContext) {
    let cx = open(cx);
    choose_in_menu(cx, "Music");
    assert_eq!(active(cx), "Music");
}

#[gpui::test]
fn new_makes_a_profile_from_the_one_in_use(cx: &mut TestAppContext) {
    let cx = open(cx);
    choose_in_menu(cx, "New");
    answer(cx, "Anime");
    assert!(names(cx).contains(&"Anime".to_owned()), "{:?}", names(cx));
    assert_eq!(toast_messages(cx), ["Made the profile “Anime” from “Movies”."]);
}

#[gpui::test]
fn rename_renames_the_profile_in_use_and_keeps_a_blank_one(cx: &mut TestAppContext) {
    let cx = open(cx);
    choose_in_menu(cx, "Rename “Movies”…");
    answer(cx, "Films");
    assert_eq!(active(cx), "Films");
    choose_in_menu(cx, "Rename “Films”…");
    tap(cx, "backspace");
    answer(cx, "");
    assert_eq!(active(cx), "Films");
    let expected = ["Renamed to “Films”.", "A profile needs a name; it keeps the old one."];
    assert_eq!(toast_messages(cx), expected);
}

#[gpui::test]
fn delete_asks_first_then_switches_away(cx: &mut TestAppContext) {
    let cx = open(cx);
    choose_in_menu(cx, "Delete “Movies”…");
    let main = main_view(cx);
    assert!(main.read_with(cx, |main, cx| main.modals.read(cx).is_open()), "it asks");
    // Focus starts on Cancel; Delete is next.
    tap(cx, "tab");
    draw(cx);
    tap(cx, "enter");
    draw(cx);
    assert!(!names(cx).contains(&"Movies".to_owned()));
    let now = active(cx);
    assert_eq!(toast_messages(cx), [format!("Deleted “Movies”. Now using “{now}”.")]);
}

#[gpui::test]
fn delete_without_confirmations_deletes_at_once(cx: &mut TestAppContext) {
    let cx = open(cx);
    cx.update(|_, cx| settings::update(cx, |s| s.prefs.startup.show_dialogs = false));
    choose_in_menu(cx, "Delete “Movies”…");
    assert_eq!(names(cx).len(), 3);
}
