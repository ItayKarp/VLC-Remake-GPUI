//! Headless tests of restoring and saving the session. The player isn't started here (it needs
//! libVLC), so they cover the restores that load nothing and what a save would record.

use std::path::PathBuf;
use std::time::Duration;

use gpui::{App, TestAppContext, VisualTestContext};

use crate::app::media::Media;
use crate::app::request::OpenRequest;
use crate::main_window::MainWindow;
use crate::main_window::tests::main_view;
use crate::queue::Repeat;
use crate::session::{self, Snapshot};
use crate::settings;

fn file(name: &str) -> Media {
    Media::File(PathBuf::from(format!("/no/such/folder/{name}")))
}

/// A session that ended while its second item played.
fn crashed() -> Snapshot {
    Snapshot {
        media: vec![file("a.mkv"), file("b.mkv"), Media::Url("https://example.com/c".into())],
        current: Some(1),
        position: Duration::from_secs(90),
        repeat: Repeat::All,
        shuffle: true,
        playing: true,
    }
}

/// Opens the window for `request`, with `saved` left by the last run.
fn launch(
    cx: &mut TestAppContext,
    saved: Snapshot,
    request: OpenRequest,
) -> &mut VisualTestContext {
    launch_with(cx, saved, request, |_| {})
}

/// [`launch`], with the settings changed by `configure` first.
fn launch_with(
    cx: &mut TestAppContext,
    saved: Snapshot,
    request: OpenRequest,
    configure: impl FnOnce(&mut App),
) -> &mut VisualTestContext {
    cx.update(|cx| {
        settings::init_in_memory(cx);
        configure(cx);
        crate::theme::init(cx);
        crate::ui::init(cx);
        crate::keymap::init(cx);
        session::init_in_memory(Some(saved), cx);
    });
    let (_, cx) = cx.add_window_view(|window, cx| MainWindow::new(request, window, cx));
    cx.run_until_parked();
    cx
}

fn queued(cx: &mut VisualTestContext) -> Vec<Media> {
    let main = main_view(cx);
    main.read_with(cx, |main, _| main.queue.entries().iter().map(|e| e.media.clone()).collect())
}

#[gpui::test]
fn the_queue_comes_back_without_loading_what_was_playing_when_the_app_died(
    cx: &mut TestAppContext,
) {
    let cx = launch(cx, crashed(), OpenRequest::default());
    assert_eq!(queued(cx), crashed().media);
    let main = main_view(cx);
    main.read_with(cx, |main, _| {
        assert_eq!(main.queue.current_index(), Some(1));
        assert_eq!(main.queue.repeat(), Repeat::All);
        assert!(main.queue.is_shuffled());
        assert!(main.player.is_none(), "the crashing item must not load");
    });
    let live = cx.update(|_, cx| settings::get(cx).live);
    assert_eq!((live.repeat, live.shuffle), (Repeat::All, true), "Settings shows them");
}

#[gpui::test]
fn restored_files_that_are_not_there_are_greyed(cx: &mut TestAppContext) {
    let saved = Snapshot { playing: false, ..crashed() };
    let cx = launch(cx, saved, OpenRequest::default());
    let main = main_view(cx);
    main.read_with(cx, |main, _| {
        assert!(main.player.is_none(), "a missing file waits for Play");
        let missing = &main.queue_panel.missing;
        assert!(missing.contains(&file("a.mkv")) && missing.contains(&file("b.mkv")));
        assert_eq!(missing.len(), 2, "a URL isn't checked");
    });
}

#[gpui::test]
fn opening_files_at_launch_replaces_the_last_session(cx: &mut TestAppContext) {
    let cx = launch(cx, crashed(), OpenRequest::replace(vec![file("new.mkv")]));
    let main = main_view(cx);
    main.read_with(cx, |main, _| assert_eq!(main.queue.repeat(), Repeat::Off));
}

#[gpui::test]
fn nothing_is_restored_when_the_setting_is_off(cx: &mut TestAppContext) {
    let off = |cx: &mut App| settings::update(cx, |s| s.prefs.player.restore_session = false);
    let cx = launch_with(cx, crashed(), OpenRequest::default(), off);
    assert_eq!(queued(cx), []);
}

#[gpui::test]
fn nothing_is_restored_while_history_is_off(cx: &mut TestAppContext) {
    let off = |cx: &mut App| settings::update(cx, |s| s.prefs.privacy.save_recently_played = false);
    let cx = launch_with(cx, crashed(), OpenRequest::default(), off);
    assert_eq!(queued(cx), []);
}

#[gpui::test]
fn a_save_records_the_queue_and_its_play_order(cx: &mut TestAppContext) {
    let cx = launch(cx, crashed(), OpenRequest::default());
    let main = main_view(cx);
    main.update(cx, |main, cx| {
        main.queue.set_repeat(Repeat::One);
        main.queue.set_shuffle(false);
        let saved = main.snapshot(cx);
        assert_eq!(saved.media, crashed().media);
        assert_eq!(saved.current, Some(1));
        assert_eq!((saved.repeat, saved.shuffle, saved.playing), (Repeat::One, false, false));
        assert_eq!(saved.position, Duration::ZERO, "nothing is loaded");
    });
}
