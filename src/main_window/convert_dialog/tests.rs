//! Window tests of the Convert dialog and of Convert in the main window. The app's
//! conversions are given two running already, so what Start queues waits its turn and no
//! conversion really runs.

use std::path::PathBuf;

use gpui::{AppContext as _, Entity, TestAppContext, VisualTestContext};

use super::ConvertDialog;
use crate::app::media::Media;
use crate::convert::{self, Conversions, Destination, Preset, Status};
use crate::keymap::actions::Convert;
use crate::main_window::device_dialog::tests::{draw, open_view, play_mock};
use crate::main_window::tests::{main_view, open};
use crate::opening::scratch::scratch_dir;

/// A file to convert, which needn't exist.
fn file(name: &str) -> Media {
    Media::File(PathBuf::from("/videos").join(name))
}

/// Conversions with two running (in name only: no thread converts), so any more wait.
fn busy(cx: &mut TestAppContext) -> Entity<Conversions> {
    cx.new(|_| {
        let mut conversions = Conversions::default();
        let queue = conversions.queue_mut();
        for name in ["one.mkv", "two.mkv"] {
            queue.add(file(name), Preset::default(), &Destination::NextToSource, |_| false);
        }
        queue.start_due();
        conversions
    })
}

/// Opens the dialog holding `sources` on `conversions`.
fn open_dialog(
    cx: &mut TestAppContext,
    sources: Vec<Media>,
    conversions: Entity<Conversions>,
) -> (Entity<ConvertDialog>, &mut VisualTestContext) {
    open_view(cx, move |window, cx| ConvertDialog::new(sources, conversions, window, cx))
}

fn sources(dialog: &Entity<ConvertDialog>, cx: &mut VisualTestContext) -> Vec<Media> {
    dialog.read_with(cx, |dialog, _| dialog.sources.clone())
}

#[gpui::test]
fn files_are_listed_once_and_can_be_taken_off(cx: &mut TestAppContext) {
    let conversions = busy(cx);
    let (dialog, cx) = open_dialog(cx, vec![file("a.mkv"), file("b.mkv")], conversions);
    dialog.update(cx, |dialog, cx| dialog.remove_source(0, cx));
    dialog.update(cx, |dialog, cx| dialog.remove_source(5, cx));
    dialog.update_in(cx, |_, window, cx| ConvertDialog::add_files(window, cx));
    cx.simulate_path_prompt_response(|_| {
        Some(["b.mkv", "c.mkv"].map(|name| PathBuf::from("/videos").join(name)).to_vec())
    });
    draw(cx);
    assert_eq!(sources(&dialog, cx), [file("b.mkv"), file("c.mkv")]);
}

#[gpui::test]
fn start_queues_the_files_in_the_format_and_folder_picked(cx: &mut TestAppContext) {
    let conversions = busy(cx);
    let (dialog, cx) = open_dialog(cx, vec![file("a.mkv")], conversions.clone());
    let flac = Preset::ALL.iter().position(|&preset| preset == Preset::Flac).expect("FLAC");
    let presets = dialog.read_with(cx, |dialog, _| dialog.presets.clone());
    presets.update(cx, |list, cx| list.set_selected(flac, cx));
    let folder = scratch_dir();
    dialog.update_in(cx, |dialog, window, cx| dialog.set_destination(true, window, cx));
    let picked = folder.clone();
    cx.simulate_path_prompt_response(move |_| Some(vec![picked]));
    draw(cx);
    dialog.update_in(cx, ConvertDialog::start);
    draw(cx);
    assert_eq!(sources(&dialog, cx), [], "the list empties, the dialog stays on the queue");
    conversions.read_with(cx, |conversions, _| {
        let queued = conversions.queue().jobs().last().expect("a job");
        assert_eq!(queued.source, file("a.mkv"));
        assert_eq!((queued.preset, &queued.status), (Preset::Flac, &Status::Waiting));
        assert_eq!(queued.output, folder.join("a.flac"));
        assert_eq!(conversions.choice, (Preset::Flac, Destination::Folder(folder.clone())));
    });
    dialog.update_in(cx, |dialog, window, cx| dialog.set_destination(false, window, cx));
    let destination = dialog.read_with(cx, |dialog, _| dialog.destination.clone());
    assert_eq!(destination, Destination::NextToSource);
}

#[gpui::test]
fn start_without_files_does_nothing(cx: &mut TestAppContext) {
    let conversions = busy(cx);
    let (dialog, cx) = open_dialog(cx, Vec::new(), conversions.clone());
    dialog.update_in(cx, ConvertDialog::start);
    assert_eq!(conversions.read_with(cx, |conversions, _| conversions.queue().jobs().len()), 2);
}

#[gpui::test]
fn the_queue_shows_every_conversion_as_it_goes(cx: &mut TestAppContext) {
    let conversions = busy(cx);
    conversions.update(cx, |conversions, _| {
        let queue = conversions.queue_mut();
        queue.set_progress(0, 0.5);
        let at = Destination::NextToSource;
        let ids = ["done.mkv", "failed.mkv", "cancelled.mkv", "waiting.mkv"]
            .map(|name| queue.add(file(name), Preset::default(), &at, |_| false));
        queue.finish(1, Ok(()));
        queue.finish(ids[0], Ok(()));
        queue.finish(ids[1], Err("The disk is full".into()));
        queue.cancel(ids[2]);
    });
    let (dialog, cx) = open_dialog(cx, Vec::new(), conversions.clone());
    conversions.update(cx, Conversions::clear_finished);
    draw(cx);
    let left = dialog.read_with(cx, |dialog, cx| dialog.conversions.read(cx).queue().unfinished());
    assert_eq!(left, 2, "the running one and the waiting one stay");
    conversions.update(cx, Conversions::cancel_all);
    draw(cx);
    let left = conversions.read_with(cx, |conversions, _| conversions.queue().unfinished());
    assert_eq!(left, 0, "all done");
}

/// The main window with the app's conversions started, and the window's view.
fn main_with_conversions(cx: &mut TestAppContext) -> &mut VisualTestContext {
    let cx = open(cx);
    cx.update(|_, cx| convert::init(cx));
    cx
}

fn has_dialog(cx: &mut VisualTestContext) -> bool {
    let main = main_view(cx);
    main.read_with(cx, |main, cx| main.modals.read(cx).is_open())
}

#[gpui::test]
fn convert_opens_the_dialog(cx: &mut TestAppContext) {
    let cx = main_with_conversions(cx);
    play_mock(cx);
    cx.dispatch_action(Convert);
    draw(cx);
    assert!(has_dialog(cx));
}

#[gpui::test]
fn closing_asks_first_while_conversions_run(cx: &mut TestAppContext) {
    let cx = main_with_conversions(cx);
    let main = main_view(cx);
    let weak = main.downgrade();
    let may = cx.update(|window, cx| super::may_close(&weak, window, cx));
    assert!(may, "nothing runs");
    cx.update(|_, cx| {
        let conversions = Conversions::global(cx).expect("started");
        conversions.update(cx, |conversions, _| {
            let queue = conversions.queue_mut();
            queue.add(file("a.mkv"), Preset::default(), &Destination::NextToSource, |_| false);
        });
    });
    let may = cx.update(|window, cx| super::may_close(&weak, window, cx));
    draw(cx);
    assert!(!may);
    assert!(has_dialog(cx), "it asks whether to cancel and quit");
}

#[gpui::test]
fn quitting_asks_first_while_conversions_run(cx: &mut TestAppContext) {
    let cx = main_with_conversions(cx);
    cx.update(|_, cx| {
        let conversions = Conversions::global(cx).expect("started");
        conversions.update(cx, |conversions, _| {
            let queue = conversions.queue_mut();
            queue.add(file("a.mkv"), Preset::default(), &Destination::NextToSource, |_| false);
        });
    });
    let app: &mut TestAppContext = cx;
    app.update(super::ask_before_quitting);
    draw(cx);
    assert!(has_dialog(cx));
}
