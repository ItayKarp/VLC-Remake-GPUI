//! The seek bar as drawn over the mock's media: a live stream's LIVE chip, a network stream's
//! buffered band, and scrubbing by dragging along the track.

use std::time::Duration;

use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px};

use super::Shown;
use crate::engine::backend::BackendEvent;
use crate::engine::mock::MockMedia;
use crate::main_window::now_playing::tests::{draw, open_playing, play_mock};
use crate::main_window::tests::{main_view, open};

fn shown(cx: &mut VisualTestContext) -> Shown {
    main_view(cx).read_with(cx, |main, cx| main.shown_seek(None, cx))
}

#[gpui::test]
fn a_live_stream_says_live(cx: &mut TestAppContext) {
    let cx = open(cx);
    let _playing = play_mock(cx, MockMedia { duration: None, ..MockMedia::default() });
    draw(cx);
    let shown = shown(cx);
    assert!(shown.live);
    assert_eq!(shown.times, None);
}

#[gpui::test]
fn a_network_stream_shows_what_is_buffered(cx: &mut TestAppContext) {
    let (cx, playing) = open_playing(cx);
    playing.mock.emit(BackendEvent::Position(Duration::from_secs(6)));
    playing.mock.emit(BackendEvent::Buffering(50.));
    draw(cx);
    let shown = shown(cx);
    assert_eq!(shown.times, Some((Duration::from_secs(6), Duration::from_secs(60))));
    assert!((shown.fraction - 0.1).abs() < 1e-3);
    let buffered = shown.buffered.expect("a network stream buffers");
    assert!(buffered >= shown.fraction, "{buffered}");
    assert!(!shown.live);
}

#[gpui::test]
fn dragging_along_the_track_scrubs_then_seeks(cx: &mut TestAppContext) {
    let (cx, playing) = open_playing(cx);
    draw(cx);
    let main = main_view(cx);
    let bounds = main.read_with(cx, |main, _| main.seek_bar.track.bounds());
    let bounds = bounds.expect("the seek bar is drawn");
    let y = bounds.center().y;
    let start = point(bounds.left() + px(2.), y);
    let middle = point(bounds.left() + bounds.size.width / 2., y);
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    draw(cx);
    assert!(main.read_with(cx, |main, _| main.seek_bar.is_scrubbing()));
    cx.simulate_mouse_move(middle, MouseButton::Left, Modifiers::none());
    draw(cx);
    let fraction = shown(cx).fraction;
    assert!((fraction - 0.5).abs() < 0.05, "the handle follows the drag: {fraction}");
    cx.simulate_mouse_up(middle, MouseButton::Left, Modifiers::none());
    draw(cx);
    assert!(!main.read_with(cx, |main, _| main.seek_bar.is_scrubbing()));
    let position = playing.player.read_with(cx, |player, _| player.position());
    assert!(position > Duration::from_secs(25) && position < Duration::from_secs(35));
}
