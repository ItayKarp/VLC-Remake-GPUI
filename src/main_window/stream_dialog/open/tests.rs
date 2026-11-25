//! Window tests of Stream in the main window, on a silent mock player: the command opens the
//! dialog only with something open, starting streams the media open from where it is or
//! queues and plays other media streaming, and a stream that couldn't start ends.

use std::net::{Ipv4Addr, SocketAddr};

use gpui::{Entity, TestAppContext, VisualTestContext};

use crate::app::media::Media;
use crate::engine::backend::{BackendEvent, Command};
use crate::engine::mock::MockHandle;
use crate::engine::player::Player;
use crate::engine::stream_out::{Contents, Encoding, Local, Protocol, StreamQuality, StreamTarget};
use crate::keymap::actions::Stream;
use crate::main_window::device_dialog::tests::{draw, mock_clip, play_mock};
use crate::main_window::tests::{main_view, open};

fn target() -> StreamTarget {
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, 8080));
    StreamTarget {
        protocol: Protocol::Http,
        address,
        viewer_host: address.ip(),
        quality: StreamQuality::default(),
        contents: Contents::Everything { subtitles: true },
        encoding: Encoding::default(),
        local: Local::PlaysHere,
    }
}

/// The main window playing the mock clip, its stream events heard as the window hears them.
fn playing(cx: &mut TestAppContext) -> (&mut VisualTestContext, Entity<Player>, MockHandle) {
    let cx = open(cx);
    let (player, mock) = play_mock(cx);
    (cx, player, mock)
}

fn has_dialog(cx: &mut VisualTestContext) -> bool {
    let main = main_view(cx);
    main.read_with(cx, |main, cx| main.modals.read(cx).is_open())
}

fn streaming(player: &Entity<Player>, cx: &mut VisualTestContext) -> Option<StreamTarget> {
    player.read_with(cx, |player, _| player.stream_target().copied())
}

/// Streams `media` to `target`, or stops with `None`, as the dialog's Stream and Stop do.
fn choose(media: Media, target: Option<StreamTarget>, cx: &mut VisualTestContext) {
    let main = main_view(cx);
    main.update_in(cx, |main, window, cx| main.choose_stream(media, target, window, cx));
    draw(cx);
}

#[gpui::test]
fn stream_needs_something_open(cx: &mut TestAppContext) {
    let cx = open(cx);
    cx.dispatch_action(Stream);
    draw(cx);
    assert!(!has_dialog(cx), "a toast says to play something first");
}

#[gpui::test]
fn stream_opens_the_dialog_for_what_plays(cx: &mut TestAppContext) {
    let (cx, _, _) = playing(cx);
    cx.dispatch_action(Stream);
    draw(cx);
    assert!(has_dialog(cx));
}

#[gpui::test]
fn the_media_open_streams_from_where_it_is_until_stopped(cx: &mut TestAppContext) {
    let (cx, player, mock) = playing(cx);
    mock.take_commands();
    choose(mock_clip(), Some(target()), cx);
    assert_eq!(streaming(&player, cx), Some(target()));
    assert_eq!(mock.take_commands(), [Command::Stream(Some(target()))], "no reopening");
    let main = main_view(cx);
    let running = main.read_with(cx, |main, cx| main.running_stream(&mock_clip(), cx));
    assert_eq!(running, Some(target()));
    main.update_in(cx, |main, window, cx| main.show_stream(mock_clip(), window, cx));
    draw(cx);
    assert!(has_dialog(cx), "the dialog shows the stream running");
    choose(mock_clip(), None, cx);
    assert_eq!(streaming(&player, cx), None);
}

#[gpui::test]
fn other_media_is_played_next_streaming(cx: &mut TestAppContext) {
    let (cx, player, _) = playing(cx);
    let other = Media::Url("mock://other".into());
    choose(other.clone(), Some(target()), cx);
    player.read_with(cx, |player, _| {
        assert_eq!(player.media(), Some(&other));
        assert_eq!(player.stream_target(), Some(&target()));
    });
    let main = main_view(cx);
    let queued =
        main.read_with(cx, |main, _| main.queue.entries().iter().any(|entry| entry.media == other));
    assert!(queued, "it went in the queue");
}

#[gpui::test]
fn a_stream_that_couldnt_start_ends(cx: &mut TestAppContext) {
    let (cx, player, mock) = playing(cx);
    choose(mock_clip(), Some(target()), cx);
    mock.emit(BackendEvent::StreamFailed);
    draw(cx);
    assert_eq!(streaming(&player, cx), None);
}
