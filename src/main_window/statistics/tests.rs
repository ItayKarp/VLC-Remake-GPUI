//! Show Statistics in the main window: the overlay is drawn only while it is on and media is
//! open, and the player's counts are read for it once a second meanwhile.

use std::time::Duration;

use gpui::{TestAppContext, VisualTestContext};

use crate::engine::backend::Command;
use crate::engine::mock::MockMedia;
use crate::main_window::now_playing::tests::{Playing, draw, play_mock};
use crate::main_window::tests::{main_view, open};

/// Whether the overlay was drawn in the last frame.
fn overlay_drawn(cx: &mut VisualTestContext) -> bool {
    draw(cx);
    cx.debug_bounds("statistics-overlay").is_some()
}

/// Turns Show Statistics on or off, as its toggle in the settings panel does.
fn show_statistics(cx: &mut VisualTestContext, shown: bool) {
    main_view(cx).update(cx, |main, cx| main.set_show_statistics(shown, cx));
    draw(cx);
}

/// How many times the player was asked for its counts since last asked.
fn reads(playing: &Playing) -> usize {
    let commands = playing.mock.take_commands();
    commands.iter().filter(|command| **command == Command::ReadStatistics).count()
}

#[gpui::test]
fn with_nothing_open_the_overlay_waits_for_media(cx: &mut TestAppContext) {
    let cx = open(cx);
    show_statistics(cx, true);
    cx.executor().advance_clock(Duration::from_secs(2));
    assert!(!overlay_drawn(cx), "nothing to count");
    assert!(main_view(cx).read_with(cx, |main, _| main.shows_statistics()));

    let playing = play_mock(cx, MockMedia::default());
    cx.executor().advance_clock(Duration::from_secs(1));
    assert!(overlay_drawn(cx));
    assert!(reads(&playing) >= 1, "the media's counts are read once it plays");
    let counted = playing.player.read_with(cx, |player, _| player.statistics().is_some());
    assert!(counted, "the counts the overlay shows came back");
}

#[gpui::test]
fn turning_it_off_hides_the_overlay_and_stops_reading(cx: &mut TestAppContext) {
    let cx = open(cx);
    let playing = play_mock(cx, MockMedia::default());
    assert!(!overlay_drawn(cx), "off by default");
    show_statistics(cx, true);
    assert!(overlay_drawn(cx));
    show_statistics(cx, false);
    reads(&playing);
    cx.executor().advance_clock(Duration::from_secs(3));
    assert!(!overlay_drawn(cx));
    assert_eq!(reads(&playing), 0);
}

#[gpui::test]
fn closing_the_window_stops_reading(cx: &mut TestAppContext) {
    let cx = open(cx);
    let playing = play_mock(cx, MockMedia::default());
    show_statistics(cx, true);
    reads(&playing);
    cx.update(|window, _| window.remove_window());
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_secs(3));
    cx.run_until_parked();
    assert_eq!(reads(&playing), 0);
}
