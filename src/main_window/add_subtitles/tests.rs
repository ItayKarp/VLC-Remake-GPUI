//! Window tests of the Add Subtitles dialog on real folders: a folder's videos pair with their
//! episodes' subtitles, Save copies a checked file next to the video when asked, and
//! OpenSubtitles asks before a search leaves the machine.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui::{Entity, TestAppContext, VisualTestContext};

use super::model::{Origin, Target};
use super::search::Search;
use super::{AddSubtitlesDialog, Body, Saved};
use crate::opening::scratch::scratch_dir;

const SRT: &str = "1\n00:00:01,000 --> 00:00:02,000\nHello\n";

fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().expect("a folder")).unwrap();
    std::fs::write(path, contents).unwrap();
}

/// Opens the dialog on `target`, collecting what Save hands over.
fn open(
    cx: &mut TestAppContext,
    target: Target,
) -> (Entity<AddSubtitlesDialog>, Rc<RefCell<Vec<Saved>>>, &mut VisualTestContext) {
    cx.update(|cx| {
        crate::settings::init_in_memory(cx);
        crate::theme::init(cx);
        crate::ui::init(cx);
    });
    let saves: Rc<RefCell<Vec<Saved>>> = Rc::default();
    let log = saves.clone();
    let (dialog, cx) = cx.add_window_view(move |window, cx| {
        let log = log.clone();
        AddSubtitlesDialog::new(target, move |saved, _, _| log.borrow_mut().push(saved), window, cx)
    });
    draw(cx);
    (dialog, saves, cx)
}

/// Settles the dialog's tasks and paints a frame, as the window would.
fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
}

#[gpui::test]
fn a_folders_videos_pair_with_their_episodes(cx: &mut TestAppContext) {
    let folder = scratch_dir();
    for episode in ["The_Wand_S01E01", "The_Wand_S01E02"] {
        write(&folder.join(format!("{episode}.mp4")), "video");
    }
    write(&folder.join("the.wand.1x02.en.srt"), SRT);
    write(&folder.join("The Wand S01E01.srt"), SRT);
    let (dialog, _, cx) = open(cx, Target::Folder(folder.clone()));
    let chosen: Vec<Vec<PathBuf>> = dialog.read_with(cx, |dialog, _| match &dialog.body {
        Body::Pairs(pairs) => pairs.choices.iter().map(|choice| choice.chosen.clone()).collect(),
        _ => panic!("a folder lists pairs"),
    });
    assert_eq!(
        chosen,
        [[folder.join("The Wand S01E01.srt")], [folder.join("the.wand.1x02.en.srt")]]
    );
}

#[gpui::test]
fn save_copies_a_checked_file_next_to_the_video(cx: &mut TestAppContext) {
    let folder = scratch_dir();
    let video = folder.join("Movie.mkv");
    write(&video, "video");
    let subtitle = folder.join("subs/Movie_English.srt");
    write(&subtitle, SRT);
    let (dialog, saves, cx) = open(cx, Target::File(video));
    dialog.update_in(cx, |dialog, window, cx| {
        dialog.source = Some(folder.join("subs"));
        dialog.reload(window, cx);
    });
    draw(cx);
    dialog.update_in(cx, |dialog, window, cx| {
        let Body::Files(files) = &mut dialog.body else { panic!("a file lists files") };
        assert_eq!(files.candidates, std::slice::from_ref(&subtitle));
        files.choice.toggle(&subtitle);
        dialog.copying.on = true;
        dialog.save(window, cx);
    });
    cx.run_until_parked();
    let copy = folder.join("Movie.en.srt");
    assert_eq!(std::fs::read_to_string(&copy).unwrap(), SRT);
    let saves = saves.borrow();
    assert_eq!(saves[0].outcomes[0].linked, Ok(copy));
}

#[gpui::test]
fn opensubtitles_asks_before_sending_anything(cx: &mut TestAppContext) {
    let video = scratch_dir().join("The_Wand_S01E01.mkv");
    write(&video, "video");
    let (dialog, _, cx) = open(cx, Target::File(video));
    dialog.update_in(cx, |dialog, window, cx| {
        dialog.origin = Origin::OpenSubtitles;
        dialog.reload(window, cx);
    });
    draw(cx);
    dialog.read_with(cx, |dialog, _| {
        assert!(matches!(dialog.body, Body::Search(Search::Consent)));
        assert!(dialog.loading.is_none(), "nothing is searched before the user allows it");
    });
}
