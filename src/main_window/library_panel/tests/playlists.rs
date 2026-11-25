//! Headless tests of a saved playlist row's menu (S4.8.3): what it offers for a playlist with
//! items and an empty one, and what Play, Shuffle, Add to Queue, Favorite, Export Playlist and
//! Properties & Metadata do.

use std::path::PathBuf;

use futures::FutureExt as _;
use gpui::{TestAppContext, VisualTestContext};

use super::super::listing::Body;
use super::super::playlist_menu;
use super::support::{choose, dialog_open, entries, find, labels, queued, show, with_library};
use crate::app::media::Media;
use crate::db::activity::Favorite;
use crate::db::playlists::Summary;
use crate::history::tests::support::TestHistory;
use crate::library::favorites;
use crate::main_window::tests::{main_view, tap, toast_messages};
use crate::navigation::route::Route;
use crate::opening::scratch::scratch_dir;
use crate::playlists::{self, Draft};
use crate::ui::menu::MenuItem;

/// Two video files in a new folder.
pub(super) fn two_files() -> Vec<PathBuf> {
    let dir = scratch_dir();
    let files = vec![dir.join("a.mkv"), dir.join("b.mkv")];
    for file in &files {
        std::fs::write(file, b"x").expect("file");
    }
    files
}

/// Saves a playlist named `title` holding `items`, and answers its summary.
pub(super) fn save(
    cx: &mut VisualTestContext,
    history: &mut TestHistory,
    title: &str,
    items: Vec<Media>,
) -> Summary {
    let draft = Draft { title: title.into(), thumbnail: None, items };
    let task = cx.update(|_, cx| playlists::create(&draft, cx));
    history.settle(cx);
    let title = task.now_or_never().flatten().expect("saved");
    cx.update(|_, cx| playlists::summaries(cx).iter().find(|s| s.title == title).cloned())
        .expect("listed")
}

/// The library with "Mix", holding two files.
fn with_mix(cx: &mut TestAppContext) -> (&mut VisualTestContext, TestHistory, Summary) {
    let (cx, mut history) = with_library(cx, Vec::new());
    let files = two_files().into_iter().map(Media::File).collect();
    let mix = save(cx, &mut history, "Mix", files);
    (cx, history, mix)
}

/// The playlist menu of `summary`.
fn menu(cx: &mut VisualTestContext, summary: &Summary) -> Vec<MenuItem> {
    let main = main_view(cx);
    cx.update(|_, cx| playlist_menu::items(&main.downgrade(), std::slice::from_ref(summary), cx))
}

/// The listed rows' names, in order.
pub(super) fn rows(cx: &mut VisualTestContext) -> Vec<String> {
    let main = main_view(cx);
    cx.update(|_, cx| {
        let main = main.read(cx);
        let Body::Items(items) = main.listing(cx) else { panic!("the place was read") };
        items.rows.iter().map(|row| row.label().to_string()).collect()
    })
}

#[gpui::test]
fn a_playlist_row_has_the_playlist_menu(cx: &mut TestAppContext) {
    let (cx, mut history, mix) = with_mix(cx);
    let empty = save(cx, &mut history, "Empty", Vec::new());
    show(cx, &mut history, Route::Playlists(None));
    assert_eq!(rows(cx), ["Empty", "Mix"]);
    let at = cx.debug_bounds("library-row-1").expect("drawn").center();
    cx.simulate_mouse_down(at, gpui::MouseButton::Right, gpui::Modifiers::none());
    cx.simulate_mouse_up(at, gpui::MouseButton::Right, gpui::Modifiers::none());
    cx.run_until_parked();
    let open = main_view(cx).read_with(cx, |main, _| main.library_panel.item_menu.is_some());
    assert!(open);
    let expected = [
        "Play",
        "Favorite",
        "Shuffle",
        "Export Playlist",
        "Add to Queue",
        "Add Subtitle Tracks",
        "Properties & Metadata",
    ];
    assert_eq!(labels(&menu(cx, &mix)), expected);
    let disabled: Vec<String> = entries(&menu(cx, &empty))
        .into_iter()
        .filter_map(|(label, enabled)| (!enabled).then_some(label))
        .collect();
    assert_eq!(disabled, ["Play", "Shuffle", "Add to Queue", "Add Subtitle Tracks"]);
}

#[gpui::test]
fn add_to_queue_play_and_shuffle_read_the_items(cx: &mut TestAppContext) {
    let (cx, mut history, mix) = with_mix(cx);
    let items = menu(cx, &mix);
    choose(cx, &mut history, &items, &["Add to Queue"]);
    assert_eq!(queued(cx), ["a.mkv", "b.mkv"]);
    choose(cx, &mut history, &items, &["Play"]);
    assert_eq!(queued(cx), ["a.mkv", "b.mkv"], "Play replaces the queue");
    choose(cx, &mut history, &items, &["Shuffle"]);
    let shuffled = main_view(cx).read_with(cx, |main, _| main.queue.is_shuffled());
    assert!(shuffled);
    assert_eq!(queued(cx).len(), 2);
}

#[gpui::test]
fn favorite_marks_the_playlist(cx: &mut TestAppContext) {
    let (cx, mut history, mix) = with_mix(cx);
    let items = menu(cx, &mix);
    choose(cx, &mut history, &items, &["Favorite"]);
    let marked = cx.update(|_, cx| favorites::all(&[Favorite::Playlist(mix.id)], cx));
    assert!(marked);
    let items = menu(cx, &mix);
    assert!(matches!(find(&items, &["Favorite"]), MenuItem::Check { checked: true, .. }));
}

#[gpui::test]
fn properties_and_subtitles_open_their_dialogs(cx: &mut TestAppContext) {
    let (cx, mut history, mix) = with_mix(cx);
    let items = menu(cx, &mix);
    choose(cx, &mut history, &items, &["Properties & Metadata"]);
    assert!(dialog_open(cx));
    tap(cx, "escape");
    assert!(!dialog_open(cx));
    choose(cx, &mut history, &items, &["Add Subtitle Tracks"]);
    assert!(dialog_open(cx));
}

#[gpui::test]
fn export_writes_the_playlist_where_it_is_saved(cx: &mut TestAppContext) {
    let (cx, mut history, mix) = with_mix(cx);
    let items = menu(cx, &mix);
    choose(cx, &mut history, &items, &["Export Playlist"]);
    assert!(dialog_open(cx));
    tap(cx, "enter");
    let target = scratch_dir().join("Mix.m3u8");
    let chosen = target.clone();
    cx.simulate_new_path_selection(move |_| Some(chosen));
    history.settle(cx);
    let written = std::fs::read_to_string(&target).expect("exported");
    assert!(written.contains("a.mkv") && written.contains("b.mkv"), "{written}");
    assert_eq!(toast_messages(cx).last().map(String::as_str), Some("Exported to “Mix.m3u8”."));
}

#[gpui::test]
fn an_export_that_cant_be_written_says_why(cx: &mut TestAppContext) {
    let (cx, mut history, mix) = with_mix(cx);
    let items = menu(cx, &mix);
    choose(cx, &mut history, &items, &["Export Playlist"]);
    tap(cx, "enter");
    let nowhere = scratch_dir().join("missing").join("Mix.m3u8");
    cx.simulate_new_path_selection(move |_| Some(nowhere));
    history.settle(cx);
    assert_eq!(toast_messages(cx).len(), 1, "the failure is shown");
}
