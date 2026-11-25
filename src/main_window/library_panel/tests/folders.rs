//! Headless tests of adding to the library: a folder row's menu (S4.8.2), which copies files
//! and folders into the folder and shows its Info, the rail's "+" menu (S4.5.7) and the "+ Add"
//! row (S4.6.1), which add a folder to the library and show it.

use std::path::PathBuf;

use gpui::{Modifiers, TestAppContext, VisualTestContext};

use super::super::{add_menu, folder_menu};
use super::support::{choose, dialog_open, entries, show, toasts, with_library};
use crate::history::tests::support::TestHistory;
use crate::main_window::MainWindow;
use crate::main_window::tests::{main_view, tap};
use crate::navigation::route::{MusicRoute, Route};
use crate::opening::scratch::scratch_dir;
use crate::settings;
use crate::ui::menu::MenuItem;

/// The library showing a folder that holds "Season 1".
fn with_season(cx: &mut TestAppContext) -> (&mut VisualTestContext, TestHistory, PathBuf) {
    let (cx, mut history) = with_library(cx, Vec::new());
    let dir = scratch_dir();
    let season = dir.join("Season 1");
    std::fs::create_dir(&season).expect("subfolder");
    show(cx, &mut history, Route::Videos(Some(dir)));
    (cx, history, season)
}

/// The folder menu of `folders`.
fn menu(cx: &mut VisualTestContext, folders: &[PathBuf]) -> Vec<MenuItem> {
    folder_menu::items(&main_view(cx).downgrade(), folders)
}

/// Answers the open path prompt with `paths`, and lets what follows finish.
fn pick(cx: &mut VisualTestContext, history: &mut TestHistory, paths: Vec<PathBuf>) {
    assert!(cx.did_prompt_for_paths(), "it asks");
    cx.simulate_path_prompt_response(move |_| Some(paths));
    history.settle(cx);
}

fn route(cx: &mut VisualTestContext) -> Route {
    let main = main_view(cx);
    cx.update(|_, cx| main.read(cx).library_panel.history.current().clone())
}

#[gpui::test]
fn add_file_copies_the_picked_files_into_the_folder(cx: &mut TestAppContext) {
    let (cx, mut history, season) = with_season(cx);
    let source = scratch_dir().join("Pilot.mkv");
    std::fs::write(&source, b"x").expect("file");
    let items = menu(cx, std::slice::from_ref(&season));
    choose(cx, &mut history, &items, &["Add File"]);
    pick(cx, &mut history, vec![source]);
    assert!(season.join("Pilot.mkv").exists());
    assert_eq!(toasts(cx), ["Copied 1 file into “Season 1”."]);
}

#[gpui::test]
fn add_folder_copies_a_folder_and_says_what_failed(cx: &mut TestAppContext) {
    let (cx, mut history, season) = with_season(cx);
    let extras = scratch_dir().join("Extras");
    std::fs::create_dir(&extras).expect("folder");
    let items = menu(cx, std::slice::from_ref(&season));
    choose(cx, &mut history, &items, &["Add Folder"]);
    pick(cx, &mut history, vec![extras]);
    assert!(season.join("Extras").is_dir());
    choose(cx, &mut history, &items, &["Add File"]);
    pick(cx, &mut history, vec![scratch_dir().join("gone.mkv")]);
    let said = toasts(cx);
    assert!(said.last().is_some_and(|toast| toast.starts_with("Couldn't copy")), "{said:?}");
}

#[gpui::test]
fn several_folders_resync_but_add_one_at_a_time(cx: &mut TestAppContext) {
    let (cx, mut history, season) = with_season(cx);
    let other = season.with_file_name("Season 2");
    let items = menu(cx, &[season, other]);
    let enabled: Vec<String> =
        entries(&items).into_iter().filter_map(|(label, on)| on.then_some(label)).collect();
    assert_eq!(enabled, ["Resync"]);
    choose(cx, &mut history, &items, &["Resync"]);
    let main = main_view(cx);
    assert_eq!(cx.update(|_, cx| main.read(cx).listed_count(cx)), 1, "the folder is listed again");
}

#[gpui::test]
fn info_and_edit_subtitle_tracks_open_their_dialogs(cx: &mut TestAppContext) {
    let (cx, mut history, season) = with_season(cx);
    let items = menu(cx, std::slice::from_ref(&season));
    choose(cx, &mut history, &items, &["Info"]);
    assert!(dialog_open(cx));
    tap(cx, "escape");
    assert!(!dialog_open(cx));
    choose(cx, &mut history, &items, &["Edit Subtitle Tracks"]);
    assert!(dialog_open(cx));
}

#[gpui::test]
fn the_rails_add_menu_adds_a_folder_to_the_library(cx: &mut TestAppContext) {
    let (cx, mut history) = with_library(cx, Vec::new());
    let main = main_view(cx);
    main.update_in(cx, MainWindow::toggle_add_menu);
    cx.run_until_parked();
    let open = |cx: &mut VisualTestContext| {
        main.read_with(cx, |main, _| main.library_panel.add_menu.is_some())
    };
    assert!(open(cx));
    main.update_in(cx, MainWindow::toggle_add_menu);
    assert!(!open(cx), "the button closes it again");

    let folder = scratch_dir();
    let items = main.update(cx, |_, cx| add_menu::items(&cx.entity().downgrade(), cx));
    choose(cx, &mut history, &items, &["Add Folder to Library…"]);
    pick(cx, &mut history, vec![folder.clone()]);
    assert_eq!(route(cx), Route::Videos(Some(folder.clone())));
    let folders = cx.update(|_, cx| settings::get(cx).local.media_folders.clone());
    assert_eq!(folders.iter().filter(|known| **known == folder).count(), 1);
}

#[gpui::test]
fn the_add_row_of_songs_adds_a_music_folder(cx: &mut TestAppContext) {
    let (cx, mut history) = with_library(cx, Vec::new());
    show(cx, &mut history, Route::Music(MusicRoute::Songs(None)));
    let at = cx.debug_bounds("library-row-0").expect("the + Add row is drawn").center();
    cx.simulate_click(at, Modifiers::none());
    let folder = scratch_dir();
    pick(cx, &mut history, vec![folder.clone()]);
    assert_eq!(route(cx), Route::Music(MusicRoute::Songs(Some(folder.clone()))));
    let folders = cx.update(|_, cx| settings::get(cx).local.music_folders.clone());
    assert!(folders.contains(&folder));
}

#[gpui::test]
fn add_network_stream_opens_its_dialog(cx: &mut TestAppContext) {
    let (cx, mut history) = with_library(cx, Vec::new());
    show(cx, &mut history, Route::Network(None));
    let at = cx.debug_bounds("library-row-0").expect("the row is drawn").center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    assert!(dialog_open(cx));
}
