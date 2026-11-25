//! The sleep timer in a window playing the mock's clip: it counts down, fades the sound over its
//! last seconds without moving the volume, stops playback and says so, and puts the sound back
//! when turned off.

use std::time::{Duration, SystemTime};

use gpui::{TestAppContext, VisualTestContext};

use super::rules::{Choice, Timer};
use super::{FADE_TICK, TICK};
use crate::engine::backend::BackendEvent;
use crate::engine::state::PlaybackState;
use crate::main_window::now_playing::tests::{Playing, open_playing, osd};
use crate::main_window::tests::main_view;

/// The dropdown's entry and the line under it.
fn shown(cx: &mut VisualTestContext) -> (Choice, Option<String>) {
    main_view(cx).read_with(cx, |main, cx| (main.sleep_choice(), main.sleep_hint(cx)))
}

fn pick(cx: &mut VisualTestContext, choice: Choice) {
    main_view(cx).update(cx, |main, cx| main.pick_sleep_timer(choice, cx));
    cx.run_until_parked();
}

/// Moves the wall clock on, as far as the running timer is concerned: it is now due in `left`.
fn due_in(cx: &mut VisualTestContext, left: Duration) {
    main_view(cx).update(cx, |main, _| {
        assert!(matches!(main.sleep_timer.timer, Some(Timer::At(_))), "a timer of minutes runs");
        main.sleep_timer.timer = Some(Timer::At(SystemTime::now() + left));
    });
}

/// Lets the timer take its next look at the clock.
fn tick(cx: &mut VisualTestContext, after: Duration) {
    cx.executor().advance_clock(after);
    cx.run_until_parked();
}

fn fade(cx: &mut VisualTestContext, playing: &Playing) -> f32 {
    playing.player.read_with(cx, |player, _| player.fade())
}

fn state(cx: &mut VisualTestContext, playing: &Playing) -> PlaybackState {
    playing.player.read_with(cx, |player, _| player.state())
}

#[gpui::test]
fn a_timer_counts_down_fades_then_stops_playback(cx: &mut TestAppContext) {
    let (cx, playing) = open_playing(cx);
    let volume = playing.player.read_with(cx, |player, _| player.volume());
    pick(cx, Choice::Minutes(15));
    assert_eq!(shown(cx), (Choice::Minutes(15), Some("Stops in 15 min".into())));
    due_in(cx, Duration::from_secs(5));
    tick(cx, TICK);
    let faded = fade(cx, &playing);
    assert!(faded > 0.3 && faded < 0.6, "half way through the fade: {faded}");
    let control = playing.player.read_with(cx, |player, _| player.volume());
    assert_eq!(control, volume, "the volume control doesn't move");
    due_in(cx, Duration::ZERO);
    tick(cx, FADE_TICK);
    assert_eq!(state(cx, &playing), PlaybackState::Stopped);
    assert_eq!(shown(cx), (Choice::Off, None));
    assert!((fade(cx, &playing) - 1.0).abs() < f32::EPSILON, "the sound comes back");
    assert_eq!(osd(cx), "Sleep timer: playback stopped");
}

#[gpui::test]
fn turning_the_timer_off_puts_the_sound_back(cx: &mut TestAppContext) {
    let (cx, playing) = open_playing(cx);
    pick(cx, Choice::Minutes(30));
    due_in(cx, Duration::from_secs(2));
    tick(cx, TICK);
    assert!(fade(cx, &playing) < 0.5);
    pick(cx, Choice::Off);
    assert_eq!(shown(cx), (Choice::Off, None));
    assert!((fade(cx, &playing) - 1.0).abs() < f32::EPSILON);
    tick(cx, TICK);
    assert_eq!(state(cx, &playing), PlaybackState::Playing, "an ended timer does nothing");
}

#[gpui::test]
fn the_end_of_item_timer_fades_with_the_item_and_stops_at_its_end(cx: &mut TestAppContext) {
    let (cx, playing) = open_playing(cx);
    pick(cx, Choice::EndOfItem);
    assert_eq!(shown(cx), (Choice::EndOfItem, Some("Stops when this item ends".into())));
    tick(cx, TICK);
    assert!((fade(cx, &playing) - 1.0).abs() < f32::EPSILON, "a minute left: no fade yet");
    playing.mock.emit(BackendEvent::Position(Duration::from_secs(55)));
    tick(cx, TICK);
    let faded = fade(cx, &playing);
    assert!(faded > 0.3 && faded < 0.6, "five seconds left: {faded}");
    playing.mock.emit(BackendEvent::State(PlaybackState::Ended));
    cx.run_until_parked();
    assert_eq!(state(cx, &playing), PlaybackState::Stopped);
    assert_eq!(shown(cx), (Choice::Off, None));
    assert_eq!(osd(cx), "Sleep timer: playback stopped");
}

#[gpui::test]
fn custom_waits_for_minutes_typed(cx: &mut TestAppContext) {
    let (cx, _playing) = open_playing(cx);
    pick(cx, Choice::Minutes(45));
    pick(cx, Choice::Custom);
    assert_eq!(shown(cx), (Choice::Custom, None), "Custom ends the running timer");
    main_view(cx).update(cx, |main, cx| main.start_sleep_minutes(20, Choice::Custom, cx));
    assert_eq!(shown(cx), (Choice::Custom, Some("Stops in 20 min".into())));
}

#[gpui::test]
fn without_media_the_timer_waits_on_the_clock(cx: &mut TestAppContext) {
    let cx = crate::main_window::tests::open(cx);
    pick(cx, Choice::EndOfItem);
    tick(cx, TICK);
    assert_eq!(shown(cx), (Choice::EndOfItem, Some("Stops when this item ends".into())));
    pick(cx, Choice::Minutes(60));
    due_in(cx, Duration::ZERO);
    tick(cx, TICK);
    assert_eq!(shown(cx), (Choice::Off, None), "it fires with nothing to stop");
}
