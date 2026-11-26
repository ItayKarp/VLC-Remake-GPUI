//! Opening from the window, on a player with the mock backend: the Open dialogs, paste, what
//! is reported about the items, and cancelling.

use std::path::PathBuf;

use gpui::{ClipboardItem, TestAppContext, VisualTestContext};

use crate::app::media::Media;
use crate::app::request::OpenRequest;
use crate::engine::mock::MockMedia;
use crate::keymap::actions::{OpenFile, OpenFolder, OpenFromClipboard};
use crate::main_window::tests::toast_messages;
use crate::main_window::tests::{main_view, open};
use crate::main_window::video_menu::tests::support::with_mock_player;
use crate::opening::scratch::scratch_dir;

/// The window with a player on the mock backend, nothing opened yet.
fn window(cx: &mut TestAppContext) -> &mut VisualTestContext {
    let cx = open(cx);
    with_mock_player(cx, MockMedia::default());
    cx
}

/// A new file named `name` in a folder of its own.
fn file(name: &str) -> PathBuf {
    let path = scratch_dir().join(name);
    std::fs::write(&path, "video").unwrap();
    path
}

fn queued(cx: &mut VisualTestContext) -> Vec<String> {
    let main = main_view(cx);
    main.read_with(cx, |main, _| {
        main.queue.entries().iter().map(|entry| entry.media.display_name()).collect()
    })
}

#[gpui::test]
fn open_file_and_open_folder_ask_then_play(cx: &mut TestAppContext) {
    let cx = window(cx);
    let movie = file("Movie.mkv");
    cx.dispatch_action(OpenFile);
    cx.simulate_path_prompt_response(|options| {
        assert!(options.files && !options.directories);
        Some(vec![movie.clone()])
    });
    cx.run_until_parked();
    assert_eq!(queued(cx), ["Movie.mkv"]);

    let folder = file("Episode.mkv").parent().expect("a folder").to_path_buf();
    cx.dispatch_action(OpenFolder);
    cx.simulate_path_prompt_response(|options| {
        assert!(options.directories);
        Some(vec![folder.clone()])
    });
    cx.run_until_parked();
    assert_eq!(queued(cx), ["Episode.mkv"]);
}

#[gpui::test]
fn pasting_a_copied_path_opens_it(cx: &mut TestAppContext) {
    let cx = window(cx);
    let movie = file("Pasted.mkv");
    cx.write_to_clipboard(ClipboardItem::new_string(movie.display().to_string()));
    cx.dispatch_action(OpenFromClipboard);
    cx.run_until_parked();
    assert_eq!(queued(cx), ["Pasted.mkv"]);
}

#[gpui::test]
fn pasting_nothing_says_what_to_copy(cx: &mut TestAppContext) {
    let cx = window(cx);
    cx.write_to_clipboard(ClipboardItem::new_string("   ".into()));
    cx.dispatch_action(OpenFromClipboard);
    cx.run_until_parked();
    assert_eq!(toast_messages(cx), ["Copy a file path or a URL first, then paste it here."]);
}

#[gpui::test]
fn folders_without_media_and_files_that_arent_media_are_reported(cx: &mut TestAppContext) {
    let cx = window(cx);
    let clip = file("Clip.mkv");
    let notes = clip.with_file_name("notes.txt");
    std::fs::write(&notes, "notes").unwrap();
    let empty = scratch_dir().join("Extras");
    std::fs::create_dir_all(&empty).unwrap();
    let items = [clip, notes, empty].map(Media::from).to_vec();
    let main = main_view(cx);
    main.update_in(cx, |main, window, cx| main.open(OpenRequest::replace(items), window, cx));
    cx.run_until_parked();
    assert_eq!(queued(cx), ["Clip.mkv"]);
    let toasts = toast_messages(cx);
    assert!(toasts.contains(&"No media files in “Extras”.".to_owned()), "{toasts:?}");
    assert!(toasts.contains(&"1 file skipped: it isn't media.".to_owned()), "{toasts:?}");
}

#[gpui::test]
fn a_cancelled_open_plays_nothing(cx: &mut TestAppContext) {
    let cx = window(cx);
    let movie = Media::from(file("Late.mkv"));
    let main = main_view(cx);
    main.update_in(cx, |main, window, cx| {
        main.open(OpenRequest::replace(vec![movie]), window, cx);
        assert_eq!(main.jobs.running[0].label.as_ref(), "Late.mkv");
        main.cancel_opening(cx);
    });
    cx.run_until_parked();
    assert_eq!(queued(cx), Vec::<String>::new());
}
