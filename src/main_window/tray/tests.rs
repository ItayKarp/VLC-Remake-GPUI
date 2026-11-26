//! The tray menu's commands run through the main window, and Show VLC brings back a window the
//! tray hid.

use gpui::{TestAppContext, VisualTestContext};

use crate::app::media::Media;
use crate::app::request::{OpenRequest, Placement};
use crate::engine::mock::MockMedia;
use crate::main_window::MainWindow;
use crate::main_window::tests::{hide_in_tray, main_view, open};
use crate::main_window::video_menu::tests::support::{player, playing};
use crate::opening::scratch::scratch_dir;
use crate::tray::TrayCommand;

fn run(cx: &mut VisualTestContext, command: TrayCommand) {
    let main = main_view(cx);
    main.update_in(cx, |main, window, cx| main.run_tray_command(command, window, cx));
    cx.run_until_parked();
}

fn current(cx: &mut VisualTestContext) -> Option<usize> {
    let main = main_view(cx);
    main.read_with(cx, |main, _| main.queue.current_index())
}

#[gpui::test]
fn play_pause_previous_and_next_drive_the_queue(cx: &mut TestAppContext) {
    let (cx, _mock) = playing(cx, MockMedia::default());
    let second = scratch_dir().join("Second.mkv");
    std::fs::write(&second, "video").unwrap();
    let main = main_view(cx);
    main.update_in(cx, |main, window, cx| {
        let request = OpenRequest::placed(vec![Media::from(second)], Placement::Enqueue);
        main.open(request, window, cx);
    });
    cx.run_until_parked();

    run(cx, TrayCommand::PlayPause);
    let player = player(cx);
    assert!(!player.read_with(cx, |player, _| player.wants_to_play()), "paused");
    run(cx, TrayCommand::Next);
    assert_eq!(current(cx), Some(1));
    run(cx, TrayCommand::Previous);
    assert_eq!(current(cx), Some(0));
    run(cx, TrayCommand::Quit);
    assert_eq!(current(cx), Some(0), "Quit is the app's");
}

#[gpui::test]
fn show_vlc_brings_back_a_hidden_window(cx: &mut TestAppContext) {
    let cx = open(cx);
    let main = main_view(cx);
    hide_in_tray(cx);
    assert!(main.read_with(cx, |main, _| main.is_hidden_in_tray()));
    run(cx, TrayCommand::ShowVlc);
    assert!(!main.read_with(cx, |main, _| main.is_hidden_in_tray()));
}

#[gpui::test]
fn closing_without_a_tray_icon_closes(cx: &mut TestAppContext) {
    let cx = open(cx);
    let main = main_view(cx);
    let hid = main.update_in(cx, MainWindow::hide_to_tray);
    assert!(!hid, "no tray icon shows, so the window closes");
}
