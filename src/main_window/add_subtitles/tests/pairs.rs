//! To This Folder, From Computer: the pairs' pickers check, clear and add subtitles, the list
//! warns when the folder fits no video, and a copy's taken name offers its choices.

use std::path::{Path, PathBuf};

use gpui::{Entity, Modifiers, MouseButton, TestAppContext, VisualTestContext};

use super::{SRT, draw, open, write};
use crate::main_window::add_subtitles::model::Target;
use crate::main_window::add_subtitles::{AddSubtitlesDialog, Body};
use crate::opening::scratch::scratch_dir;
use crate::subtitle_links::copy::OnConflict;

/// A folder of two episodes, its subtitles named `subtitles`.
fn episodes(subtitles: &[&str]) -> PathBuf {
    let folder = scratch_dir();
    for video in ["Ep1.mkv", "Ep2.mkv"] {
        write(&folder.join(video), "video");
    }
    for subtitle in subtitles {
        write(&folder.join(subtitle), SRT);
    }
    folder
}

/// Each video's chosen subtitles' names.
fn chosen(dialog: &Entity<AddSubtitlesDialog>, cx: &mut VisualTestContext) -> Vec<Vec<String>> {
    dialog.read_with(cx, |dialog, _| {
        let Body::Pairs(pairs) = &dialog.body else { panic!("a folder lists pairs") };
        let names = |paths: &[PathBuf]| paths.iter().map(|path| name(path)).collect();
        pairs.choices.iter().map(|choice| names(&choice.chosen)).collect()
    })
}

fn name(path: &Path) -> String {
    path.file_name().expect("a file").to_string_lossy().into_owned()
}

fn click(cx: &mut VisualTestContext, id: String) {
    // Debug ids are looked up as `'static`; a test's few ids live on until it ends anyway.
    let id: &'static str = Box::leak(id.into_boxed_str());
    let at = cx.debug_bounds(id).unwrap_or_else(|| panic!("{id} isn't drawn")).center();
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    draw(cx);
}

/// The open picker's or conflict menu's entries.
fn menu(dialog: &Entity<AddSubtitlesDialog>, cx: &mut VisualTestContext) -> Vec<String> {
    dialog.read_with(cx, |dialog, cx| {
        let (_, menu, _) = dialog.menu.as_ref().expect("a menu is open");
        menu.read(cx).entries().into_iter().map(|(label, _)| label).collect()
    })
}

/// Picks the open menu's entry `index` (counting separators out) with the keyboard.
fn pick(cx: &mut VisualTestContext, index: usize) {
    for _ in 0..=index {
        cx.simulate_keystrokes("down");
    }
    cx.simulate_keystrokes("enter");
    draw(cx);
}

#[gpui::test]
fn the_picker_checks_clears_and_chooses_files(cx: &mut TestAppContext) {
    let folder = episodes(&["Ep1.srt", "Ep2.en.srt"]);
    let (dialog, _, cx) = open(cx, Target::Folder(folder));
    assert_eq!(chosen(&dialog, cx), [["Ep1.srt"], ["Ep2.en.srt"]]);

    click(cx, "add-subtitles-pick-1".into());
    let entries = menu(&dialog, cx);
    assert_eq!(entries[entries.len() - 2..], ["No Match", "Choose File…"]);
    let other = entries.iter().position(|entry| entry == "Ep1.srt").expect("every subtitle");
    pick(cx, other);
    assert_eq!(chosen(&dialog, cx)[1], ["Ep2.en.srt", "Ep1.srt"]);

    click(cx, "add-subtitles-pick-1".into());
    let no_match = menu(&dialog, cx).len() - 2;
    pick(cx, no_match);
    assert!(chosen(&dialog, cx)[1].is_empty(), "No Match");

    let elsewhere = scratch_dir().join("Ep2.fr.srt");
    write(&elsewhere, SRT);
    click(cx, "add-subtitles-pick-1".into());
    let choose_file = menu(&dialog, cx).len() - 1;
    pick(cx, choose_file);
    cx.simulate_path_prompt_response(|_| Some(vec![elsewhere.clone()]));
    draw(cx);
    assert_eq!(chosen(&dialog, cx)[1], ["Ep2.fr.srt"]);
    click(cx, "add-subtitles-pick-1".into());
    assert!(menu(&dialog, cx).contains(&"Ep2.fr.srt".to_owned()), "a file from elsewhere");
}

#[gpui::test]
fn a_folder_without_fitting_subtitles_says_so(cx: &mut TestAppContext) {
    let (dialog, _, cx) = open(cx, Target::Folder(episodes(&["Unrelated.Movie.srt"])));
    assert_eq!(chosen(&dialog, cx), [Vec::<String>::new(), Vec::new()]);
    let paired_any = dialog.read_with(cx, |dialog, _| match &dialog.body {
        Body::Pairs(pairs) => pairs.paired_any,
        _ => panic!("pairs"),
    });
    assert!(!paired_any);
    let (dialog, _, cx) = open(&mut cx.cx, Target::Folder(episodes(&[])));
    assert_eq!(chosen(&dialog, cx).len(), 2);
}

#[gpui::test]
fn a_taken_copy_name_offers_overwrite_keep_both_and_skip(cx: &mut TestAppContext) {
    let folder = episodes(&[]);
    let subs = folder.join("subs");
    write(&subs.join("Ep1.en.srt"), SRT);
    write(&folder.join("Ep1.en.srt"), "an older copy");
    let (dialog, _, cx) = open(cx, Target::Folder(folder.clone()));
    dialog.update_in(cx, |dialog, window, cx| {
        dialog.source = Some(subs.clone());
        dialog.reload(window, cx);
    });
    draw(cx);
    assert_eq!(chosen(&dialog, cx)[0], ["Ep1.en.srt"]);
    dialog.update(cx, |dialog, cx| {
        dialog.copying.on = true;
        dialog.check_conflicts(cx);
    });
    draw(cx);
    let pair = (folder.join("Ep1.mkv"), subs.join("Ep1.en.srt"));
    let conflict = |dialog: &Entity<AddSubtitlesDialog>, cx: &mut VisualTestContext| {
        dialog.read_with(cx, |dialog, _| dialog.copying.conflicts.get(&pair).copied())
    };
    assert_eq!(conflict(&dialog, cx), Some(OnConflict::KeepBoth));

    click(cx, format!("conflict-{}", subs.join("Ep1.en.srt").display()));
    assert_eq!(menu(&dialog, cx), ["Overwrite", "Keep both", "Skip"]);
    pick(cx, 2);
    assert_eq!(conflict(&dialog, cx), Some(OnConflict::Skip));
}

#[gpui::test]
fn a_row_that_couldnt_be_saved_says_why(cx: &mut TestAppContext) {
    let folder = episodes(&["Ep1.srt"]);
    let (dialog, _, cx) = open(cx, Target::Folder(folder.clone()));
    let pair = (folder.join("Ep1.mkv"), folder.join("Ep1.srt"));
    dialog.update(cx, |dialog, cx| {
        dialog.errors.insert(pair.clone(), "The disk is full.".into());
        cx.notify();
    });
    draw(cx);
    let additions = dialog.read_with(cx, |dialog, _| dialog.additions());
    assert_eq!(additions, [pair]);
}
