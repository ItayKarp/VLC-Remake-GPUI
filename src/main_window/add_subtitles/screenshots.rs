//! Screenshot tests of the dialog on real folders against `dialogs/add_subtitles/*`: a folder's
//! fifteen episodes paired with theirs (`…_01.png`), and one episode's subtitles in six
//! languages, two checked (`…_02.png`), and OpenSubtitles' results, one checked (`…_03.png`).

use std::path::Path;

use gpui::{AppContext as _, Context, Entity, IntoElement, Render, Window, div, prelude::*, px};

use super::model::{Origin, Target};
use super::search::Search;
use super::{AddSubtitlesDialog, Body, body};
use crate::engine::sidecar;
use crate::opening::scratch::scratch_dir;
use crate::screenshot_test::{self, Shot};
use crate::subtitle_links;
use crate::subtitle_links::opensubtitles::Found;

/// The design PNGs' size, in logical pixels, and where the dialog sits in them.
const DESIGN_SIZE: (f32, f32) = (1078., 830.);
const DIALOG_TOP: f32 = 7.;
/// The share of pixels allowed to differ: the text, as the design uses Inter, the source
/// folder's path (the test's own folder), the footer's "Copy next to video", which the design
/// doesn't have, and the design's drop shadow.
const MAX_MISMATCH: f64 = 0.08;
const SRT: &str = "1\n00:00:01,000 --> 00:00:02,000\nHello\n";

/// The dialog where the design draws it.
struct DialogShot {
    dialog: Entity<AddSubtitlesDialog>,
}

impl Render for DialogShot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_start()
            .justify_center()
            .pt(px(DIALOG_TOP))
            .child(self.dialog.clone())
    }
}

/// The list for `target`, read on this thread, with nothing linked.
fn read_now(target: &Target) -> Body {
    let videos = match target {
        Target::File(video) => vec![video.clone()],
        Target::Folder(folder) => subtitle_links::media_in(folder),
        Target::Playlist { .. } => Vec::new(),
    };
    let source = videos[0].parent().expect("in a folder").to_path_buf();
    let linked = vec![Vec::new(); videos.len()];
    body::build(target, &videos, linked, sidecar::in_folder(&source), &[])
}

fn write(path: &Path) {
    std::fs::write(path, SRT).unwrap();
}

/// Renders the dialog on `target` once its list is read, letting `prepare` change it first,
/// against design `number`.
fn check(
    name: &'static str,
    number: u32,
    target: Target,
    prepare: impl FnOnce(&mut AddSubtitlesDialog) + 'static,
) {
    let shot = Shot {
        name,
        design: format!(
            "docs/ui/dialogs/add_subtitles/add_subtitles_to_playlist_folder_{number:02}.png"
        ),
        design_origin: (0, 0),
        window: DESIGN_SIZE,
        skip_top: 0.,
        max_mismatch: MAX_MISMATCH,
    };
    let failure = screenshot_test::check(&shot, move |window, cx| {
        let dialog = cx.new(|cx| {
            AddSubtitlesDialog::new(target, |_, _, _| {}, window, cx).without_window_margin()
        });
        // The window reads the list in the background; read it here so the frame shows it.
        dialog.update(cx, |dialog, _| {
            dialog.loading = None;
            dialog.body = read_now(dialog.target());
            prepare(dialog);
        });
        cx.new(|_| DialogShot { dialog })
    });
    assert!(failure.is_none(), "{}", failure.unwrap_or_default());
}

#[test]
fn a_folders_episodes_pair_as_designed() {
    let folder = scratch_dir();
    for episode in 1..=15 {
        std::fs::write(folder.join(format!("The_Wand_S01E{episode:02}.mp4")), b"v").unwrap();
        write(&folder.join(format!("The_Wand_S01E{episode:02}.srt")));
    }
    check("add-subtitles-folder", 1, Target::Folder(folder), |_| {});
}

#[test]
fn a_files_languages_list_as_designed() {
    let folder = scratch_dir();
    let video = folder.join("The_Wand_S01E01.mp4");
    std::fs::write(&video, b"v").unwrap();
    let languages = ["English", "Spanish", "Japanese", "German", "Dutch", "French"];
    for language in languages {
        write(&folder.join(format!("The_Wand_S01E01_{language}.srt")));
    }
    check("add-subtitles-file", 2, Target::File(video), move |dialog| {
        let Body::Files(files) = &mut dialog.body else { return };
        files.candidates.sort_by_key(|path| {
            languages.iter().position(|language| path.to_string_lossy().contains(language))
        });
        for checked in &files.candidates.clone()[..2] {
            files.choice.toggle(checked);
        }
    });
}

#[test]
fn opensubtitles_results_list_as_designed() {
    let video = scratch_dir().join("The_Wand_S01E01.mp4");
    std::fs::write(&video, b"v").unwrap();
    check("add-subtitles-opensubtitles", 3, Target::File(video), |dialog| {
        let found = |id: &str, name: &str, downloads| Found {
            id: id.to_owned(),
            name: name.to_owned(),
            release: String::new(),
            language: "en".to_owned(),
            rating: None,
            downloads,
            matches_file: false,
        };
        let results = vec![
            found("1", "The_Wand_S01E01_English.srt", 1_204),
            found("2", "The_Wand_S01E01_EN.srt", 310),
        ];
        dialog.origin = Origin::OpenSubtitles;
        dialog.body = Body::Search(Search::Found { results, checked: ["1".to_owned()].into() });
    });
}
