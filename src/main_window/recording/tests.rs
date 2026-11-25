//! Recording what the mock plays: starting into the chosen folder, the badge while it records,
//! seeking refused meanwhile, and the toast that says where the file went, or why there is none.

use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui::{TestAppContext, VisualTestContext};

use super::*;
use crate::engine::backend::{BackendEvent, Command};
use crate::engine::values::SeekTarget;
use crate::main_window::now_playing::tests::{Playing, draw, open_playing, osd};
use crate::main_window::tests::{main_view, open};
use crate::main_window::tests::{toast_levels, toast_messages};
use crate::opening::scratch::scratch_dir;

/// The window playing the mock's clip, recordings going to a folder of the test's own.
fn playing_into_folder(cx: &mut TestAppContext) -> (&mut VisualTestContext, Playing, PathBuf) {
    let (cx, playing) = open_playing(cx);
    let folder = scratch_dir();
    let setting = folder.to_string_lossy().into_owned();
    cx.update(|_, cx| settings::update(cx, |s| s.local.recordings_folder = setting));
    (cx, playing, folder)
}

fn toggle(cx: &mut VisualTestContext) {
    main_view(cx).update(cx, MainWindow::toggle_recording);
    draw(cx);
}

/// Starts recording into `folder`, which the mock says it does.
fn start(cx: &mut VisualTestContext, playing: &Playing, folder: &Path) {
    toggle(cx);
    let commands = playing.mock.take_commands();
    assert!(commands.contains(&Command::Record(Some(folder.to_path_buf()))), "{commands:?}");
}

fn messages(cx: &mut VisualTestContext) -> Vec<String> {
    toast_messages(cx)
}

#[gpui::test]
fn a_recording_starts_shows_and_says_where_it_went(cx: &mut TestAppContext) {
    let (cx, playing, folder) = playing_into_folder(cx);
    start(cx, &playing, &folder);
    let main = main_view(cx);
    assert!(main.read_with(cx, MainWindow::is_recording));
    assert_eq!(main.read_with(cx, MainWindow::why_no_recording), None);
    cx.executor().advance_clock(TICK);
    draw(cx);
    playing.player.update(cx, |player, cx| player.seek(SeekTarget::Absolute(Duration::ZERO), cx));
    cx.run_until_parked();
    assert_eq!(osd(cx), "Can't seek while recording");
    std::fs::write(folder.join("vlc-record-clip.mp4"), b"video").expect("a scratch file");
    toggle(cx);
    assert!(playing.mock.take_commands().contains(&Command::Record(None)));
    let saved = format!("Recording saved in {}", display_path(&folder));
    assert_eq!(messages(cx), [saved]);
}

#[gpui::test]
fn a_recording_that_never_got_a_picture_says_so(cx: &mut TestAppContext) {
    let (cx, playing, folder) = playing_into_folder(cx);
    start(cx, &playing, &folder);
    main_view(cx).update(cx, MainWindow::disk_nearly_full);
    assert!(playing.mock.take_commands().contains(&Command::Record(None)));
    cx.executor().advance_clock(FIND_FOR * 2);
    draw(cx);
    let full =
        format!("The disk is full, so nothing could be recorded in {}.", display_path(&folder));
    assert_eq!(messages(cx), [full]);
}

#[gpui::test]
fn media_that_cant_be_recorded_says_so(cx: &mut TestAppContext) {
    let (cx, playing, _folder) = playing_into_folder(cx);
    let main = main_view(cx);
    main.update(cx, MainWindow::toggle_recording);
    let why = main.read_with(cx, MainWindow::why_no_recording);
    assert_eq!(why, Some("Starting to record"), "while the folder is checked");
    draw(cx);
    playing.mock.emit(BackendEvent::Recording(Recording::Refused));
    draw(cx);
    assert_eq!(messages(cx), ["This media can't be recorded."]);
}

#[gpui::test]
fn nothing_playing_records_nothing(cx: &mut TestAppContext) {
    let cx = open(cx);
    toggle(cx);
    assert_eq!(osd(cx), "No media playing");
    assert!(!main_view(cx).read_with(cx, MainWindow::is_recording));
}

#[gpui::test]
fn an_unready_folder_says_why(cx: &mut TestAppContext) {
    let (cx, _playing, folder) = playing_into_folder(cx);
    toggle(cx);
    let full = Unready::NoSpace(folder.clone());
    main_view(cx).update(cx, |main, cx| main.folder_prepared(Err(full), cx));
    let unwritable = Unready::Unwritable(std::io::Error::other("read-only"));
    main_view(cx).update(cx, |main, cx| main.show_toast(unready_toast(&unwritable), cx));
    let space =
        format!("Couldn't record: there's too little space left in {}.", display_path(&folder));
    assert_eq!(messages(cx), [space, "Couldn't record: no folder could be written.".into()]);
}

#[gpui::test]
fn the_stopped_toast_says_where_the_file_is(cx: &mut TestAppContext) {
    let cx = open(cx);
    let (folder, wanted) = (PathBuf::from("/rec"), PathBuf::from("/wanted"));
    let file = Some(folder.join("vlc-record.mp4"));
    let elsewhere = Destination { folder: folder.clone(), instead_of: Some(wanted) };
    let here = Destination { folder, instead_of: None };
    main_view(cx).update(cx, |main, cx| {
        main.show_toast(stopped_toast(file.clone(), &elsewhere, false), cx);
        main.show_toast(stopped_toast(file, &here, true), cx);
        main.show_toast(stopped_toast(None, &here, false), cx);
    });
    let shown = toast_levels(cx);
    assert_eq!(
        shown[0],
        (Level::Warning, "Couldn't record in /wanted, so the recording is in /rec".into())
    );
    assert_eq!(shown[1].0, Level::Error);
    assert!(shown[1].1.starts_with("The disk is nearly full"));
    assert!(shown[2].1.starts_with("Nothing was recorded"));
}
