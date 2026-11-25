//! Headless tests of Network (S4.11.3): its root lists "+ Add Network Stream", its categories
//! and the playlists made of links only (S4.11.2), and a recent stream plays from its row and
//! moves to the top of Recent Streams.

use gpui::{Modifiers, MouseButton, TestAppContext};
use url::Url;

use super::playlists::{rows, save, two_files};
use super::support::{queued, show, with_library};
use crate::app::media::Media;
use crate::main_window::tests::{main_view, tap};
use crate::navigation::route::{NetworkCategory, Route};
use crate::network;

#[gpui::test]
fn the_root_lists_the_categories_then_the_playlists_of_links(cx: &mut TestAppContext) {
    let (cx, mut history) = with_library(cx, Vec::new());
    save(cx, &mut history, "Radio Mix", vec![Media::Url("https://e.com/live".into())]);
    save(cx, &mut history, "Files", two_files().into_iter().map(Media::File).collect());
    show(cx, &mut history, Route::Network(None));
    assert_eq!(rows(cx), ["Add Network Stream", "Podcasts", "Radio", "Radio Mix"]);
}

#[gpui::test]
fn a_recent_stream_plays_and_moves_to_the_top(cx: &mut TestAppContext) {
    let (cx, mut history) = with_library(cx, Vec::new());
    let links: Vec<String> = two_files()
        .iter()
        .map(|file| Url::from_file_path(file).expect("an absolute path").to_string())
        .collect();
    cx.update(|_, cx| network::record_recent(&links, cx));
    show(cx, &mut history, Route::Network(None));
    assert_eq!(rows(cx)[1], "Recent Streams", "once something was opened");
    show(cx, &mut history, Route::Network(Some(NetworkCategory::Recent)));
    let listed = rows(cx);
    assert_eq!(listed.len(), 2);
    let at = cx.debug_bounds("library-row-0").expect("the row is drawn").center();
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    let main = main_view(cx);
    assert!(main.read_with(cx, |main, _| main.library_panel.item_menu.is_some()), "it has a menu");
    tap(cx, "escape");
    let at = cx.debug_bounds("library-row-1").expect("the row is drawn").center();
    cx.simulate_click(at, Modifiers::none());
    history.settle(cx);
    assert_eq!(queued(cx).len(), 1);
    assert_eq!(rows(cx)[0], listed[1], "the played stream is the most recent");
}
