//! Headless tests of the video menu's saved playlists (S3.5): what Playlist ▸ offers, and what
//! each saved playlist's Play, Add to Queue, Rename…, Duplicate, Export… and Delete… do, with
//! the toasts that say so. They live with the library's tests for its database set-up.

use gpui::{TestAppContext, VisualTestContext};

use super::playlists::{rows, save, two_files};
use super::support::{
    choose, dialog_open, entries, press_toast_and_settle, queued, show, with_library,
};
use crate::app::media::Media;
use crate::db::playlists::Summary;
use crate::history::tests::support::TestHistory;
use crate::keymap::actions::{NewPlaylist, SaveQueueAsPlaylist};
use crate::main_window::tests::{main_view, tap, toast_messages};
use crate::navigation::route::Route;
use crate::playlists;
use crate::settings;
use crate::ui::menu::MenuItem;

/// The library with "Mix", holding two files.
fn with_mix(cx: &mut TestAppContext) -> (&mut VisualTestContext, TestHistory, Summary) {
    let (cx, mut history) = with_library(cx, Vec::new());
    let files = two_files().into_iter().map(Media::File).collect();
    let mix = save(cx, &mut history, "Mix", files);
    (cx, history, mix)
}

/// Playlist ▸'s saved-playlist items.
fn menu(cx: &mut VisualTestContext) -> Vec<MenuItem> {
    let main = main_view(cx);
    main.update_in(cx, |main, _, cx| main.saved_playlist_items(&cx.entity().downgrade(), cx))
}

/// The saved playlists' titles.
fn titles(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|_, cx| playlists::summaries(cx).iter().map(|s| s.title.clone()).collect())
}

const MIX: &str = "Mix (2 items)";

#[gpui::test]
fn playlist_offers_the_saved_playlists(cx: &mut TestAppContext) {
    let (cx, _history, _) = with_mix(cx);
    let expected = [
        ("New Playlist…".to_owned(), true),
        ("Save Queue as Playlist…".to_owned(), false),
        ("Import Playlist…".to_owned(), true),
        ("Add to Playlist".to_owned(), false),
        ("Saved Playlists".to_owned(), true),
    ];
    let items = menu(cx);
    assert_eq!(entries(&items), expected);
    let MenuItem::Submenu { items: saved, .. } = &items[4] else { panic!("a submenu") };
    assert_eq!(entries(saved), [(MIX.to_owned(), true)]);
}

#[gpui::test]
fn a_saved_playlist_plays_and_queues(cx: &mut TestAppContext) {
    let (cx, mut history, _) = with_mix(cx);
    let items = menu(cx);
    choose(cx, &mut history, &items, &["Saved Playlists", MIX, "Add to Queue"]);
    assert_eq!(queued(cx), ["a.mkv", "b.mkv"]);
    choose(cx, &mut history, &items, &["Saved Playlists", MIX, "Play"]);
    assert_eq!(queued(cx), ["a.mkv", "b.mkv"], "Play replaces the queue");
    // Something plays now, so it can be added to a playlist.
    let items = menu(cx);
    choose(cx, &mut history, &items, &["Add to Playlist", "Mix"]);
    assert_eq!(toast_messages(cx).last().map(String::as_str), Some("Added to “Mix”."));
    let len = cx.update(|_, cx| playlists::summaries(cx)[0].len);
    assert_eq!(len, 3);
}

#[gpui::test]
fn duplicate_copies_under_a_numbered_title(cx: &mut TestAppContext) {
    let (cx, mut history, _) = with_mix(cx);
    let items = menu(cx);
    choose(cx, &mut history, &items, &["Saved Playlists", MIX, "Duplicate"]);
    assert_eq!(titles(cx).len(), 2);
    let said = toast_messages(cx);
    assert!(said.last().is_some_and(|toast| toast.starts_with("Made a copy named")), "{said:?}");
}

#[gpui::test]
fn rename_takes_the_typed_title(cx: &mut TestAppContext) {
    let (cx, mut history, _) = with_mix(cx);
    let items = menu(cx);
    choose(cx, &mut history, &items, &["Saved Playlists", MIX, "Rename…"]);
    assert!(dialog_open(cx));
    cx.simulate_input("Road Trip");
    tap(cx, "enter");
    history.settle(cx);
    assert_eq!(titles(cx), ["Road Trip"]);
    assert_eq!(toast_messages(cx).last().map(String::as_str), Some("Renamed to “Road Trip”."));
}

#[gpui::test]
fn a_blank_title_is_not_saved(cx: &mut TestAppContext) {
    let (cx, mut history, _) = with_mix(cx);
    let items = menu(cx);
    choose(cx, &mut history, &items, &["Saved Playlists", MIX, "Rename…"]);
    cx.simulate_input("   ");
    tap(cx, "enter");
    history.settle(cx);
    assert_eq!(titles(cx), ["Mix"]);
    let failed = "Couldn't save the change to your playlists.";
    assert_eq!(toast_messages(cx).last().map(String::as_str), Some(failed));
}

#[gpui::test]
fn delete_asks_first_and_undo_puts_it_back(cx: &mut TestAppContext) {
    let (cx, mut history, _) = with_mix(cx);
    let items = menu(cx);
    choose(cx, &mut history, &items, &["Saved Playlists", MIX, "Delete…"]);
    assert!(dialog_open(cx), "it asks first");
    // Cancel has focus first; Tab moves on to Delete.
    tap(cx, "tab");
    tap(cx, "enter");
    history.settle(cx);
    assert_eq!(titles(cx), Vec::<String>::new());
    assert_eq!(toast_messages(cx).last().map(String::as_str), Some("Deleted “Mix”."));
    press_toast_and_settle(cx, &mut history, "Undo");
    assert_eq!(titles(cx), ["Mix"]);
}

#[gpui::test]
fn without_confirmations_delete_is_at_once(cx: &mut TestAppContext) {
    let (cx, mut history, _) = with_mix(cx);
    cx.update(|_, cx| settings::update(cx, |s| s.prefs.startup.show_dialogs = false));
    let items = menu(cx);
    choose(cx, &mut history, &items, &["Saved Playlists", MIX, "Delete…"]);
    assert!(!dialog_open(cx));
    assert_eq!(titles(cx), Vec::<String>::new());
    show(cx, &mut history, Route::Playlists(None));
    assert_eq!(rows(cx), Vec::<String>::new());
}

#[gpui::test]
fn export_asks_how(cx: &mut TestAppContext) {
    let (cx, mut history, _) = with_mix(cx);
    let items = menu(cx);
    choose(cx, &mut history, &items, &["Saved Playlists", MIX, "Export…"]);
    assert!(dialog_open(cx));
}

#[gpui::test]
fn new_playlist_saves_what_is_typed(cx: &mut TestAppContext) {
    let (cx, mut history) = with_library(cx, Vec::new());
    cx.dispatch_action(NewPlaylist);
    assert!(dialog_open(cx));
    // The close button has focus first; Tab moves on to the title.
    tap(cx, "tab");
    cx.simulate_input("Evening");
    tap(cx, "enter");
    history.settle(cx);
    assert_eq!(titles(cx), ["Evening"]);
    assert_eq!(
        toast_messages(cx).last().map(String::as_str),
        Some("Saved the playlist “Evening”.")
    );
}

#[gpui::test]
fn save_queue_as_playlist_opens_the_dialog(cx: &mut TestAppContext) {
    let (cx, _history) = with_library(cx, Vec::new());
    cx.dispatch_action(SaveQueueAsPlaylist);
    assert!(dialog_open(cx));
}
