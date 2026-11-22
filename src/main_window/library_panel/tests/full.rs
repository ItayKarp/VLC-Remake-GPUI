//! Headless tests of full library mode (S4.10.2) and the folder chip (S4.10).

use gpui::{TestAppContext, VisualTestContext};

use super::browse;
use crate::main_window::MainWindow;
use crate::main_window::tests::{main_view, open, tap};
use crate::navigation::route::Route;
use crate::settings;

fn pop_out(cx: &mut VisualTestContext) {
    main_view(cx).update_in(cx, MainWindow::open_full_library);
    cx.run_until_parked();
}

fn route(cx: &mut VisualTestContext) -> Route {
    let main = main_view(cx);
    cx.update(|_, cx| main.read(cx).library_panel.history.current().clone())
}

#[gpui::test]
fn pop_out_shows_full_library_mode_without_the_control_bar(cx: &mut TestAppContext) {
    let cx = browse(cx, 3);
    let before = route(cx);
    pop_out(cx);
    let main = main_view(cx);
    let (full, chrome) = cx.update(|window, cx| {
        (main.read(cx).library_panel.is_full(), main.read(cx).chrome(window, cx))
    });
    assert!(full);
    assert!(chrome.title_bar && !chrome.controls);
    assert_eq!(route(cx), before, "the library stays where it was");
    // L goes back to the player.
    tap(cx, "l");
    assert!(!main.read_with(cx, |main, _| main.library_panel.is_open()));
}

#[gpui::test]
fn full_library_mode_remembers_its_own_route(cx: &mut TestAppContext) {
    let cx = browse(cx, 1);
    let overlay = cx.update(|_, cx| settings::get(cx).local.library_routes.overlay.clone());
    pop_out(cx);
    main_view(cx).update(cx, |main, cx| main.navigate(Route::History, cx));
    let routes = cx.update(|_, cx| settings::get(cx).local.library_routes.clone());
    assert_eq!(routes.full, "history");
    assert_eq!(routes.overlay, overlay);
}

#[gpui::test]
fn q_opens_the_miniplayers_queue_in_full_library_mode(cx: &mut TestAppContext) {
    let cx = browse(cx, 1);
    pop_out(cx);
    tap(cx, "q");
    let main = main_view(cx);
    let (docked, panel) =
        main.read_with(cx, |main, _| (main.is_miniplayer_queue_open(), main.queue_panel.is_open()));
    assert!(docked);
    assert!(!panel, "the List panel belongs to the control bar, which full library mode hides");
}

#[gpui::test]
fn the_folder_chip_opens_the_library_at_its_folder(cx: &mut TestAppContext) {
    let cx = open(cx);
    let folder = crate::opening::scratch::scratch_dir();
    let main = main_view(cx);
    main.update_in(cx, |main, window, cx| {
        let kind = crate::db::metadata::MediaKind::Video;
        main.show_library_folder(&folder, kind, window, cx);
    });
    cx.run_until_parked();
    assert!(main.read_with(cx, |main, _| main.library_panel.is_open()));
    assert_eq!(route(cx), Route::Videos(Some(folder)));
}
