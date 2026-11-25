//! Window tests of Properties & Metadata: it has a file to show only while one is open, and it
//! opens once the history thread has read the file's properties.

use gpui::TestAppContext;

use crate::app::media::Media;
use crate::history::tests::support::TestHistory;
use crate::keymap::actions::ShowProperties;
use crate::main_window::device_dialog::tests::{draw, play_mock};
use crate::main_window::tests::{main_view, open};
use crate::opening::scratch::scratch_dir;

#[gpui::test]
fn properties_show_the_file_open(cx: &mut TestAppContext) {
    let mut history = TestHistory::install(cx);
    let cx = open(cx);
    let main = main_view(cx);
    assert!(!main.read_with(cx, super::super::MainWindow::has_properties), "nothing is open");
    let (player, _) = play_mock(cx);
    assert!(!main.read_with(cx, super::super::MainWindow::has_properties), "a stream isn't a file");
    let file = scratch_dir().join("film.mkv");
    player.update(cx, |player, cx| player.open(Media::File(file), cx));
    draw(cx);
    assert!(main.read_with(cx, super::super::MainWindow::has_properties));
    cx.dispatch_action(ShowProperties);
    history.settle(cx);
    draw(cx);
    assert!(main.read_with(cx, |main, cx| main.modals.read(cx).is_open()));
}

#[gpui::test]
fn properties_need_the_history(cx: &mut TestAppContext) {
    let cx = open(cx);
    let main = main_view(cx);
    let file = scratch_dir().join("film.mkv");
    main.update_in(cx, |_, window, cx| {
        crate::main_window::MainWindow::show_properties_of(file, window, cx);
    });
    draw(cx);
    assert!(!main.read_with(cx, |main, cx| main.modals.read(cx).is_open()));
}
