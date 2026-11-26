//! The window in a test app: it remembers its page, Up and Down move through the sidebar,
//! search filters pages and skips those without matches, and Escape clears the search first.

use gpui::{Entity, TestAppContext, VisualTestContext};

use super::SettingsWindow;
use crate::settings::{self, registry::Category};
use crate::ui::text_input;

fn open(
    category: Category,
    cx: &mut TestAppContext,
) -> (Entity<SettingsWindow>, &mut VisualTestContext) {
    cx.update(|cx| {
        settings::init_in_memory(cx);
        crate::theme::init(cx);
        crate::ui::init(cx);
        super::bind_keys(cx);
    });
    let (view, cx) = cx.add_window_view(|window, cx| SettingsWindow::new(category, window, cx));
    cx.run_until_parked();
    (view, cx)
}

fn current(view: &Entity<SettingsWindow>, cx: &mut VisualTestContext) -> Category {
    view.read_with(cx, |view, _| view.category)
}

/// Types `query` in the search field.
fn search(view: &Entity<SettingsWindow>, query: &str, cx: &mut VisualTestContext) {
    let field = view.read_with(cx, |view, _| view.query.clone());
    field.update(cx, |field, cx| {
        field.set_text(query, cx);
        cx.emit(text_input::Event::Changed);
    });
    cx.run_until_parked();
}

#[gpui::test]
fn arrows_move_through_the_sidebar_and_the_page_is_remembered(cx: &mut TestAppContext) {
    let (view, cx) = open(Category::General, cx);
    cx.simulate_keystrokes("down down");
    assert_eq!(current(&view, cx), Category::Privacy);
    cx.simulate_keystrokes("up");
    assert_eq!(current(&view, cx), Category::Interface);
    assert_eq!(cx.read(|cx| settings::get(cx).local.settings_category), Category::Interface);

    cx.simulate_keystrokes("up up");
    assert_eq!(current(&view, cx), Category::General, "stops at the top");
}

#[gpui::test]
fn a_saved_page_this_build_does_not_list_opens_general(cx: &mut TestAppContext) {
    let (view, cx) = open(Category::Extensions, cx);
    let expected =
        if cfg!(feature = "extensions") { Category::Extensions } else { Category::General };
    assert_eq!(current(&view, cx), expected);
    assert_eq!(cx.read(|cx| settings::get(cx).local.settings_category), expected);
}

#[gpui::test]
fn a_search_moves_to_a_page_with_matches_and_arrows_skip_the_others(cx: &mut TestAppContext) {
    let (view, cx) = open(Category::Network, cx);
    search(&view, "passthrough", cx);
    assert_eq!(current(&view, cx), Category::Audio);

    // Audio is the only page with a match, so there is nowhere else to go.
    cx.simulate_keystrokes("down");
    assert_eq!(current(&view, cx), Category::Audio);

    assert_eq!(view.read_with(cx, |view, _| view.search.count(Category::Audio)), 1);
}

#[gpui::test]
fn escape_clears_the_search_before_anything_else(cx: &mut TestAppContext) {
    let (view, cx) = open(Category::General, cx);
    search(&view, "zzzqqq", cx);
    assert!(view.read_with(cx, |view, _| view.search.finds_nothing()));

    cx.simulate_keystrokes("escape");
    assert!(view.read_with(cx, |view, cx| view.query.read(cx).text().is_empty()));
    assert!(view.read_with(cx, |view, _| !view.search.is_active()));
}

#[gpui::test]
fn secondary_f_focuses_the_search_field(cx: &mut TestAppContext) {
    let (view, cx) = open(Category::General, cx);
    cx.simulate_keystrokes("secondary-f");
    let focused = cx.update(|window, cx| view.read(cx).query.read(cx).is_focused(window));
    assert!(focused);

    // Escape in the empty field closes the window: nothing is left to clear.
    cx.simulate_keystrokes("escape");
    assert!(cx.read(|cx| cx.windows().is_empty()));
}
