//! Window tests of the Open Disc dialog on disc folders picked with Browse…: a folder's kind
//! is picked with it, Play plays from the title and chapter typed, and an audio CD's tracks are
//! listed as they are read. No drive is looked at: the drives' discs are set by the test.

use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui::{Entity, TestAppContext, VisualTestContext};

use super::{OpenDiscDialog, Tracks};
use crate::app::media::Media;
use crate::discs::DiscKind;
use crate::engine::discs::CdTrack;
use crate::main_window::device_dialog::tests::{Handed, draw, open_view, recorder};
use crate::opening::scratch::scratch_dir;

/// Opens the dialog with no disc in the drives.
fn open(
    cx: &mut TestAppContext,
) -> (Entity<OpenDiscDialog>, Handed<Vec<Media>>, &mut VisualTestContext) {
    let (handed, on_play) = recorder();
    let (dialog, cx) = open_view(cx, move |window, cx| OpenDiscDialog::new(on_play, window, cx));
    dialog.update(cx, |dialog, cx| dialog.found(Vec::new(), cx));
    draw(cx);
    (dialog, handed, cx)
}

/// A disc folder named `name` holding the folder its kind has.
fn disc_folder(name: &str, kind_folder: &str) -> PathBuf {
    let folder = scratch_dir().join(name);
    std::fs::create_dir_all(folder.join(kind_folder)).expect("a scratch folder");
    folder
}

/// Picks `folder` with Browse….
fn browse(folder: &Path, dialog: &Entity<OpenDiscDialog>, cx: &mut VisualTestContext) {
    dialog.update_in(cx, |_, window, cx| OpenDiscDialog::browse(window, cx));
    let picked = folder.to_owned();
    cx.simulate_path_prompt_response(move |_| Some(vec![picked]));
    draw(cx);
}

fn type_in(
    field: &Entity<crate::ui::text_input::TextInput>,
    text: &str,
    cx: &mut VisualTestContext,
) {
    field.update(cx, |input, cx| input.set_text(text, cx));
    draw(cx);
}

fn play(dialog: &Entity<OpenDiscDialog>, cx: &mut VisualTestContext) {
    dialog.update_in(cx, OpenDiscDialog::play);
    draw(cx);
}

fn kind(dialog: &Entity<OpenDiscDialog>, cx: &mut VisualTestContext) -> DiscKind {
    dialog.read_with(cx, |dialog, _| dialog.kind)
}

#[gpui::test]
fn a_dvd_folder_plays_from_the_title_and_chapter_typed(cx: &mut TestAppContext) {
    let (dialog, handed, cx) = open(cx);
    dialog.update(cx, |dialog, cx| dialog.set_kind(DiscKind::Vcd, cx));
    browse(&disc_folder("MOVIE", "VIDEO_TS"), &dialog, cx);
    assert_eq!(kind(&dialog, cx), DiscKind::Dvd, "the folder's kind is picked with it");
    let (title, chapter) =
        dialog.read_with(cx, |dialog, _| (dialog.title.clone(), dialog.chapter.clone()));
    type_in(&title, "2", cx);
    type_in(&chapter, "3", cx);
    play(&dialog, cx);
    let handed = handed.borrow();
    let [items] = handed.as_slice() else { panic!("Play played once: {handed:?}") };
    assert_eq!(items.iter().map(Media::display_name).collect::<Vec<_>>(), ["MOVIE"]);
}

#[gpui::test]
fn a_blu_ray_starts_without_its_menus(cx: &mut TestAppContext) {
    let (dialog, _, cx) = open(cx);
    browse(&disc_folder("FILM", "BDMV"), &dialog, cx);
    assert_eq!(kind(&dialog, cx), DiscKind::BluRay);
    assert!(!dialog.read_with(cx, |dialog, _| dialog.menus), "Blu-ray menus need Java");
    dialog.update(cx, |dialog, cx| dialog.set_kind(DiscKind::Dvd, cx));
    assert!(dialog.read_with(cx, |dialog, _| dialog.menus), "DVD menus always work");
}

#[gpui::test]
fn without_a_disc_nothing_plays(cx: &mut TestAppContext) {
    let (dialog, handed, cx) = open(cx);
    assert!(dialog.read_with(cx, |dialog, cx| dialog.media(cx).is_empty()));
    play(&dialog, cx);
    assert!(handed.borrow().is_empty());
}

#[gpui::test]
fn an_audio_cd_lists_its_tracks_as_they_are_read(cx: &mut TestAppContext) {
    let (dialog, _, cx) = open(cx);
    dialog.update(cx, |dialog, cx| dialog.set_kind(DiscKind::AudioCd, cx));
    draw(cx);
    assert_eq!(dialog.read_with(cx, |dialog, _| dialog.tracks.clone()), Tracks::Unread);
    let track = |number: u64| CdTrack {
        title: number.is_multiple_of(2).then(|| format!("Song {number}")),
        artist: number.is_multiple_of(3).then(|| "The Band".to_owned()),
        duration: Some(Duration::from_secs(180 + number)),
    };
    let states =
        [Tracks::Reading, Tracks::Read(Vec::new()), Tracks::Read((1..=14).map(track).collect())];
    for tracks in states {
        dialog.update(cx, |dialog, cx| {
            dialog.tracks = tracks;
            cx.notify();
        });
        draw(cx);
    }
    assert!(dialog.read_with(cx, |dialog, cx| dialog.media(cx).is_empty()), "no drive to play");
}
