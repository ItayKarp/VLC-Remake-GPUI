//! To This File, From Link: what the typed link names shows as it is typed, and Save is offered
//! once there is a link.

use gpui::{Focusable as _, TestAppContext};

use super::{draw, open, write};
use crate::main_window::add_subtitles::model::{Origin, Target};
use crate::main_window::add_subtitles::{AddSubtitlesDialog, Body};
use crate::opening::scratch::scratch_dir;

#[gpui::test]
fn typing_a_link_offers_save(cx: &mut TestAppContext) {
    let video = scratch_dir().join("Movie.mkv");
    write(&video, "video");
    let (dialog, _, cx) = open(cx, Target::File(video));
    dialog.update_in(cx, |dialog, window, cx| {
        dialog.origin = Origin::Link;
        dialog.reload(window, cx);
    });
    draw(cx);
    assert!(dialog.read_with(cx, |dialog, _| matches!(dialog.body, Body::Link)));
    assert!(!dialog.read_with(cx, AddSubtitlesDialog::can_save));

    dialog.update(cx, |dialog, cx| dialog.report("Couldn't download.", true, cx));
    let field = dialog.read_with(cx, |dialog, _| dialog.controls.link_field.clone());
    cx.update(|window, cx| field.read(cx).focus_handle(cx).focus(window, cx));
    draw(cx);
    cx.simulate_input("https://example.com/Movie.en.srt");
    draw(cx);
    assert!(dialog.read_with(cx, AddSubtitlesDialog::can_save));
    assert_eq!(dialog.read_with(cx, |dialog, _| dialog.status.clone()), None, "typing clears it");
}
