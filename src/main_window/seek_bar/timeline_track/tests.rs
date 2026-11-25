//! The subtitle timeline drawn over a file's lines: the lines in the window, the part played,
//! and its buttons zooming and panning.

use std::time::Duration;

use gpui::{TestAppContext, VisualTestContext};

use super::super::subtitle_timeline::Frame;
use super::super::timeline::{Pan, Zoom};
use crate::engine::backend::BackendEvent;
use crate::engine::tracks::{Track, TrackId, TrackKind};
use crate::main_window::MainWindow;
use crate::main_window::now_playing::tests::{Playing, draw, play_mock, toasts, with_tracks};
use crate::main_window::tests::{main_view, open, tap};
use crate::opening::scratch::scratch_dir;
use crate::settings;

const SRT: &str = "1\n00:00:01,000 --> 00:00:03,000\nHello\n\n\
                   2\n00:00:20,000 --> 00:00:22,000\nAgain\n";

/// The window playing the mock's clip with an SRT file added and shown, the timeline on.
fn timeline(cx: &mut TestAppContext) -> (&mut VisualTestContext, Playing) {
    let cx = open(cx);
    let media = with_tracks(2);
    let playing = play_mock(cx, media.clone());
    let path = scratch_dir().join("Movie.srt");
    std::fs::write(&path, SRT).expect("a scratch file");
    main_view(cx).update(cx, |main, cx| main.attach_subtitles(&[path], cx));
    cx.run_until_parked();
    let mut added = media.tracks;
    added.all.push(Track::new(TrackId(7), TrackKind::Subtitle));
    added.subtitle = Some(TrackId(7));
    playing.mock.emit(BackendEvent::Tracks(added));
    cx.update(|_, cx| settings::update(cx, |s| s.prefs.interface.subtitle_timeline = true));
    draw(cx);
    (cx, playing)
}

fn frame(cx: &mut VisualTestContext) -> Option<Frame> {
    main_view(cx).read_with(cx, MainWindow::timeline_frame)
}

#[gpui::test]
fn the_timeline_draws_the_lines_around_the_playhead(cx: &mut TestAppContext) {
    let (cx, playing) = timeline(cx);
    playing.mock.emit(BackendEvent::Position(Duration::from_secs(2)));
    draw(cx);
    let frame = frame(cx).expect("the timeline shows");
    assert_eq!(frame.spans.len(), 2, "both lines are in the window");
    assert_eq!(frame.position, Duration::from_secs(2), "half the first line is played");
    playing.mock.emit(BackendEvent::Position(Duration::from_secs(25)));
    draw(cx);
    assert_eq!(frame_of(cx).position, Duration::from_secs(25), "both lines are played");
}

#[gpui::test]
fn its_buttons_zoom_and_pan_the_window(cx: &mut TestAppContext) {
    let (cx, _playing) = timeline(cx);
    let main = main_view(cx);
    let whole = frame_of(cx).visible;
    main.update(cx, |main, cx| main.zoom_timeline(Zoom::In, cx));
    draw(cx);
    let zoomed = frame_of(cx).visible;
    assert_ne!(zoomed, whole);
    assert!(main.read_with(cx, |main, cx| main.timeline_moves(cx).later));
    main.update(cx, |main, cx| main.pan_timeline(Pan::Later, cx));
    draw(cx);
    assert_ne!(frame_of(cx).visible, zoomed);
    main.update(cx, |main, cx| main.pan_timeline(Pan::Earlier, cx));
    main.update(cx, |main, cx| main.zoom_timeline(Zoom::Out, cx));
    draw(cx);
}

#[gpui::test]
fn subtitles_wait_for_a_video(cx: &mut TestAppContext) {
    let cx = open(cx);
    main_view(cx).update(cx, |main, cx| main.attach_subtitles(&[scratch_dir().join("a.srt")], cx));
    let shown: Vec<String> = toasts(cx).into_iter().map(|(_, text)| text).collect();
    assert_eq!(shown, ["Open a video first, then add subtitles."]);
}

fn frame_of(cx: &mut VisualTestContext) -> Frame {
    frame(cx).expect("the timeline shows")
}

#[gpui::test]
fn its_buttons_answer_the_keyboard(cx: &mut TestAppContext) {
    let (cx, _playing) = timeline(cx);
    let main = main_view(cx);
    let whole = frame_of(cx).visible;
    main.update(cx, |main, cx| main.zoom_timeline(Zoom::In, cx));
    draw(cx);
    // Tab from the bar reaches its first button that can be used: − at the start.
    let bar = main.read_with(cx, |main, _| main.seek_bar.focus.clone());
    cx.update(|window, cx| bar.focus(window, cx));
    draw(cx);
    cx.update(gpui::Window::focus_next);
    draw(cx);
    tap(cx, "space");
    draw(cx);
    assert_eq!(frame_of(cx).visible, whole);
}
