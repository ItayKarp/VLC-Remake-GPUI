//! Window tests of the New Playlist dialog: files picked through the drop zone, a custom
//! thumbnail checked before it is taken, Save handing over the playlist, and Cancel asking
//! before it drops files the user added.

use std::path::PathBuf;

use gpui::{AppContext as _, Entity, TestAppContext, VisualTestContext};

use super::{NewPlaylistDialog, Thumbnail};
use crate::app::media::Media;
use crate::main_window::device_dialog::tests::{Handed, draw, open_view, recorder};
use crate::opening::scratch::scratch_dir;
use crate::playlists::Draft;
use crate::ui::modal::ModalLayer;

/// Opens the dialog holding `files`, with a modal layer of its own for what it asks.
fn open(
    cx: &mut TestAppContext,
    files: Vec<Media>,
) -> (Entity<NewPlaylistDialog>, Entity<ModalLayer>, Handed<Draft>, &mut VisualTestContext) {
    let modals = cx.new(|_| ModalLayer::new());
    let layer = modals.downgrade();
    let (handed, on_save) = recorder();
    let (dialog, cx) =
        open_view(cx, move |window, cx| NewPlaylistDialog::new(files, layer, on_save, window, cx));
    (dialog, modals, handed, cx)
}

fn text(dialog: &Entity<NewPlaylistDialog>, cx: &mut VisualTestContext) -> String {
    dialog.read_with(cx, |dialog, _| dialog.drop_zone_text().to_string())
}

/// Picks `path` as the thumbnail and lets it be checked.
fn pick_thumbnail(path: PathBuf, dialog: &Entity<NewPlaylistDialog>, cx: &mut VisualTestContext) {
    dialog.update_in(cx, NewPlaylistDialog::pick_thumbnail);
    cx.simulate_path_prompt_response(move |_| Some(vec![path]));
    draw(cx);
}

fn thumbnail(dialog: &Entity<NewPlaylistDialog>, cx: &mut VisualTestContext) -> Thumbnail {
    dialog.read_with(cx, |dialog, _| dialog.thumbnail.clone())
}

fn set_title(title: &str, dialog: &Entity<NewPlaylistDialog>, cx: &mut VisualTestContext) {
    let input = dialog.read_with(cx, |dialog, _| dialog.title.clone());
    input.update(cx, |input, cx| input.set_text(title, cx));
    draw(cx);
}

#[gpui::test]
fn files_picked_in_the_drop_zone_are_counted(cx: &mut TestAppContext) {
    let (dialog, _, _, cx) = open(cx, Vec::new());
    assert_eq!(text(&dialog, cx), "Drag files into the box or click here to select files.");
    let folder = scratch_dir();
    let (song, notes) = (folder.join("song.mp3"), folder.join("notes.txt"));
    std::fs::write(&song, "audio").expect("a scratch file");
    std::fs::write(&notes, "text").expect("a scratch file");
    dialog.update_in(cx, |_, window, cx| NewPlaylistDialog::pick_files(window, cx));
    cx.simulate_path_prompt_response(move |_| Some(vec![song, notes]));
    cx.run_until_parked();
    draw(cx);
    assert_eq!(
        text(&dialog, cx),
        "1 file will be added. Drag in more or click here to select more."
    );
    let note = dialog.read_with(cx, |dialog, _| dialog.note.clone()).expect("a note");
    assert!(note.starts_with("1 item couldn't be added"), "{note}");
}

#[gpui::test]
fn a_thumbnail_is_taken_only_when_it_is_an_image(cx: &mut TestAppContext) {
    let (dialog, _, handed, cx) = open(cx, Vec::new());
    let not_an_image = scratch_dir().join("cover.png");
    std::fs::write(&not_an_image, "not a picture").expect("a scratch file");
    pick_thumbnail(not_an_image, &dialog, cx);
    assert!(matches!(thumbnail(&dialog, cx), Thumbnail::Refused(_)));
    let image = scratch_dir().join("cover.png");
    image::RgbImage::new(2, 2).save(&image).expect("a scratch image");
    pick_thumbnail(image.clone(), &dialog, cx);
    assert_eq!(thumbnail(&dialog, cx), Thumbnail::Picked(image.clone()));
    set_title("Road Trip", &dialog, cx);
    dialog.update_in(cx, NewPlaylistDialog::save);
    draw(cx);
    let draft = Draft { title: "Road Trip".into(), thumbnail: Some(image), items: Vec::new() };
    assert_eq!(*handed.borrow(), [draft]);
}

#[gpui::test]
fn save_needs_a_title(cx: &mut TestAppContext) {
    let (dialog, _, handed, cx) = open(cx, vec![Media::Url("https://example.com/a.mp4".into())]);
    assert_eq!(
        text(&dialog, cx),
        "1 file will be added. Drag in more or click here to select more."
    );
    dialog.update_in(cx, NewPlaylistDialog::save);
    assert!(handed.borrow().is_empty());
}

#[gpui::test]
fn escape_asks_before_dropping_the_files_added(cx: &mut TestAppContext) {
    let (dialog, modals, _, cx) = open(cx, Vec::new());
    dialog.update(cx, |dialog, _| dialog.added_files = true);
    let focus = dialog.read_with(cx, |dialog, _| dialog.focus_handle.clone());
    cx.update(|window, cx| focus.focus(window, cx));
    draw(cx);
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert!(modals.read_with(cx, |modals, _| modals.is_open()), "it asks first");
}
