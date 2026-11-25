//! Window tests of the Open Network Stream dialog: the links typed are read as they come, the
//! notes say what is left out and why, and Save plays the links or saves them as a playlist.

use gpui::{Entity, Focusable as _, TestAppContext, VisualTestContext};

use super::{NetworkStreamDialog, Saved};
use crate::main_window::device_dialog::tests::{Handed, draw, open_view, recorder};
use crate::ui::text_input::TextInput;

fn open(
    cx: &mut TestAppContext,
) -> (Entity<NetworkStreamDialog>, Handed<Saved>, &mut VisualTestContext) {
    let (handed, on_save) = recorder();
    let (dialog, cx) =
        open_view(cx, move |window, cx| NetworkStreamDialog::new(on_save, window, cx));
    (dialog, handed, cx)
}

/// Types `text` into `field`, as a person does.
fn type_into(field: &Entity<TextInput>, text: &str, cx: &mut VisualTestContext) {
    cx.update(|window, cx| field.read(cx).focus_handle(cx).focus(window, cx));
    draw(cx);
    cx.simulate_input(text);
    draw(cx);
}

fn fields(
    dialog: &Entity<NetworkStreamDialog>,
    cx: &mut VisualTestContext,
) -> (Entity<TextInput>, Entity<TextInput>) {
    dialog.read_with(cx, |dialog, _| (dialog.urls.clone(), dialog.name.clone()))
}

fn notes(dialog: &Entity<NetworkStreamDialog>, cx: &mut VisualTestContext) -> Vec<String> {
    dialog.read_with(cx, |dialog, cx| dialog.notes(cx).iter().map(ToString::to_string).collect())
}

#[gpui::test]
fn without_a_name_save_plays_the_links(cx: &mut TestAppContext) {
    let (dialog, handed, cx) = open(cx);
    let (urls, _) = fields(&dialog, cx);
    let link = "https://example.com/live.m3u8";
    type_into(&urls, &format!("{link} hello {link} https://me:secret@example.com/a.mp4"), cx);
    assert_eq!(
        notes(&dialog, cx),
        [
            "“hello” isn't a media link, so it is left out.",
            "1 repeated link is left out.",
            "Links with a password play, but aren't kept in Recent Streams.",
        ]
    );
    cx.simulate_keystrokes(if cfg!(target_os = "macos") { "cmd-enter" } else { "ctrl-enter" });
    draw(cx);
    let both = vec![link.to_owned(), "https://me:secret@example.com/a.mp4".to_owned()];
    assert_eq!(*handed.borrow(), [Saved::Play(both)]);
}

#[gpui::test]
fn with_a_name_save_keeps_the_links_it_may_store(cx: &mut TestAppContext) {
    let (dialog, handed, cx) = open(cx);
    let (urls, name) = fields(&dialog, cx);
    type_into(&urls, "one two https://example.com/a.mp4 https://me:secret@example.com/b.mp4", cx);
    type_into(&name, "News", cx);
    let notes = notes(&dialog, cx);
    assert_eq!(notes[0], "2 entries, like “one”, aren't media links, so they are left out.");
    assert_eq!(
        notes[1],
        "Links with a password can't be saved in a playlist, so they are left out."
    );
    dialog.update_in(cx, NetworkStreamDialog::save);
    draw(cx);
    let saved =
        Saved::Playlist { title: "News".into(), urls: vec!["https://example.com/a.mp4".into()] };
    assert_eq!(*handed.borrow(), [saved]);
}

#[gpui::test]
fn save_needs_a_link(cx: &mut TestAppContext) {
    let (dialog, handed, cx) = open(cx);
    let (urls, _) = fields(&dialog, cx);
    type_into(&urls, "nothing here", cx);
    dialog.update_in(cx, NetworkStreamDialog::save);
    draw(cx);
    assert!(handed.borrow().is_empty());
}
