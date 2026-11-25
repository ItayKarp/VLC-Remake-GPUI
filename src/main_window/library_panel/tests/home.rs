//! Headless tests of Home's page (S4.9.1): a favorite playlist fills Favorites, which shows as
//! rows in the sidebar and as tiles in full library mode, under the note that says why only
//! Favorites shows while recently played items aren't saved.

use gpui::TestAppContext;

use super::playlists::save;
use super::support::{show, with_library};
use crate::db::activity::Favorite;
use crate::library::favorites;
use crate::main_window::MainWindow;
use crate::main_window::tests::{main_view, tap};
use crate::navigation::route::{HomeSection, Route};
use crate::settings;

#[gpui::test]
fn favorites_show_on_home_in_the_sidebar_and_in_full(cx: &mut TestAppContext) {
    let (cx, mut history) = with_library(cx, Vec::new());
    cx.update(|_, cx| settings::update(cx, |s| s.prefs.privacy.save_recently_played = false));
    let mix = save(cx, &mut history, "Mix", Vec::new());
    cx.update(|_, cx| favorites::mark(vec![Favorite::Playlist(mix.id)], true, cx));
    history.settle(cx);
    show(cx, &mut history, Route::HomeSection(HomeSection::Favorites));
    show(cx, &mut history, Route::Home);
    let main = main_view(cx);
    let listed =
        |cx: &mut gpui::VisualTestContext| main.update(cx, |main, cx| main.listed_count(cx));
    assert_eq!(listed(cx), 1, "the favorite playlist");
    let heading = cx.debug_bounds("library-row-0");
    assert!(heading.is_some(), "its row is drawn");
    // The keyboard's item opens its menu under it.
    tap(cx, "down");
    tap(cx, "shift-f10");
    assert!(main.read_with(cx, |main, _| main.library_panel.item_menu.is_some()));
    tap(cx, "escape");

    main.update_in(cx, MainWindow::open_full_library);
    history.settle(cx);
    assert_eq!(listed(cx), 1, "full library mode shows the same, as tiles");
}
