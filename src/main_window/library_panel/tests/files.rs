//! Headless tests of what a media card's file menu does (S4.8.1): playing several cards,
//! adding them to a playlist, going to their source, and what a file that vanished says.

use std::path::{Path, PathBuf};

use gpui::{TestAppContext, VisualTestContext};

use super::super::file_menu;
use super::super::item_menu::FileTarget;
use super::playlists::{save, two_files};
use super::selected;
use super::support::{choose, dialog_open, queued, show, with_library};
use crate::db::metadata::MediaKind;
use crate::history::tests::support::TestHistory;
use crate::main_window::tests::{main_view, toast_messages};
use crate::navigation::route::Route;
use crate::settings;
use crate::ui::menu::MenuItem;

/// The library showing the folder of two files.
fn browsing(cx: &mut TestAppContext) -> (&mut VisualTestContext, TestHistory, Vec<PathBuf>) {
    let (cx, mut history) = with_library(cx, Vec::new());
    let files = two_files();
    let folder = files[0].parent().expect("a folder").to_path_buf();
    show(cx, &mut history, Route::Videos(Some(folder)));
    (cx, history, files)
}

/// The card of `path`, listed at `index`.
fn target(path: &Path, index: usize) -> FileTarget {
    FileTarget {
        path: path.to_path_buf(),
        media: None,
        kind: MediaKind::Video,
        watched: false,
        index,
    }
}

/// The file menu of `files`.
fn menu(cx: &mut VisualTestContext, files: &[PathBuf]) -> Vec<MenuItem> {
    let targets: Vec<_> = files.iter().enumerate().map(|(at, path)| target(path, at)).collect();
    let main = main_view(cx);
    cx.update(|_, cx| file_menu::items(&main.downgrade(), &targets, cx))
}

fn last_toast(cx: &mut VisualTestContext) -> Option<String> {
    toast_messages(cx).pop()
}

#[gpui::test]
fn play_plays_every_picked_card(cx: &mut TestAppContext) {
    let (cx, mut history, files) = browsing(cx);
    let items = menu(cx, &files);
    choose(cx, &mut history, &items, &["Play"]);
    assert_eq!(queued(cx), ["a.mkv", "b.mkv"]);
}

#[gpui::test]
fn add_to_playlist_adds_to_a_saved_one_or_a_new_one(cx: &mut TestAppContext) {
    let (cx, mut history, files) = browsing(cx);
    save(cx, &mut history, "Mix", Vec::new());
    let items = menu(cx, &files);
    choose(cx, &mut history, &items, &["Add to Playlist", "Mix"]);
    assert_eq!(last_toast(cx).as_deref(), Some("Added 2 items to “Mix”."));
    choose(cx, &mut history, &items, &["Add to Playlist", "New Playlist…"]);
    assert!(dialog_open(cx));
}

#[gpui::test]
fn go_to_source_selects_the_file_in_its_folder(cx: &mut TestAppContext) {
    let (cx, mut history, files) = browsing(cx);
    let folder = files[0].parent().expect("a folder").to_path_buf();
    cx.update(|_, cx| settings::update(cx, |s| s.local.media_folders = vec![folder.clone()]));
    show(cx, &mut history, Route::History);
    let items = menu(cx, &files[1..]);
    choose(cx, &mut history, &items, &["Go to Source"]);
    let main = main_view(cx);
    let route = cx.update(|_, cx| main.read(cx).library_panel.history.current().clone());
    assert_eq!(route, Route::Videos(Some(folder)));
    assert_eq!(selected(cx), ["b"]);
}

#[gpui::test]
fn a_vanished_file_says_it_is_not_found(cx: &mut TestAppContext) {
    let (cx, mut history, files) = browsing(cx);
    let items = menu(cx, &files[..1]);
    std::fs::remove_file(&files[0]).expect("removed");
    choose(cx, &mut history, &items, &[file_menu::show_in_folder_label()]);
    assert_eq!(last_toast(cx).as_deref(), Some("File not found"));
    choose(cx, &mut history, &items, &["Properties & Metadata"]);
    assert_eq!(toast_messages(cx), ["File not found", "File not found"]);
    // Outside every media folder, Go to Source shows the file where it is, which is gone.
    choose(cx, &mut history, &items, &["Go to Source"]);
    assert_eq!(toast_messages(cx).len(), 3);
}

#[gpui::test]
fn mark_as_watched_lists_the_folder_again(cx: &mut TestAppContext) {
    let (cx, mut history, files) = browsing(cx);
    let items = menu(cx, &files);
    choose(cx, &mut history, &items, &["Mark as Watched"]);
    let main = main_view(cx);
    assert_eq!(cx.update(|_, cx| main.read(cx).listed_count(cx)), 2);
}

#[gpui::test]
fn add_audio_tracks_asks_for_the_files(cx: &mut TestAppContext) {
    let (cx, mut history, files) = browsing(cx);
    let items = menu(cx, &files[..1]);
    choose(cx, &mut history, &items, &["Add Audio Tracks"]);
    assert!(cx.did_prompt_for_paths());
}
