//! Profiles switched from the main window: the bars follow at once.

use gpui::TestAppContext;

use crate::main_window::tests::{main_view, open};
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
