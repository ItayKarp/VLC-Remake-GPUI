//! Player tests for the Audio tab (S7.6): a delay belongs to the audio track it was set for, the
//! stereo mode reaches the backend once, and the Test pauses the media playing until the tone is
//! over.

use std::time::Duration;

use gpui::TestAppContext;

use super::Rig;
use super::tracks::{movie, open_with};
use crate::app::media::Media;
use crate::engine::audio::visualizer::VisualEffect;
use crate::engine::audio::{StereoMode, test_tone};
use crate::engine::backend::Command;
use crate::engine::mock::MockMedia;
use crate::engine::player::Player;
use crate::engine::state::PlaybackState;
use crate::engine::tracks::{TrackId, TrackKind};
use crate::engine::values::Offset;

fn hear(rig: &Rig, cx: &mut TestAppContext, track: Option<i32>) {
    rig.run(cx, |player, cx| player.select_track(TrackKind::Audio, track.map(TrackId), cx));
}

fn delay(rig: &Rig, cx: &mut TestAppContext) -> Offset {
    rig.player.read_with(cx, |player, _| player.audio_delay())
}

fn set_delay(rig: &Rig, cx: &mut TestAppContext, millis: i64) -> bool {
    rig.player.update(cx, |player, cx| player.set_audio_delay(Offset::from_millis(millis), cx))
}

fn delays(rig: &Rig) -> Vec<Offset> {
    let commands = rig.mock.take_commands();
    commands
        .into_iter()
        .filter_map(|command| match command {
            Command::SetAudioDelay(delay) => Some(delay),
            _ => None,
        })
        .collect()
}

#[gpui::test]
fn a_delay_belongs_to_the_audio_track_it_was_set_for(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    open_with(&rig, cx, movie());
    assert!(set_delay(&rig, cx, -300));
    assert_eq!(delays(&rig), [Offset::from_millis(-300)]);

    hear(&rig, cx, Some(1));
    assert_eq!(delay(&rig, cx), Offset::default());
    assert_eq!(delays(&rig), [Offset::default()]);

    hear(&rig, cx, Some(2));
    assert_eq!(delay(&rig, cx), Offset::from_millis(-300));
    assert_eq!(delays(&rig), [Offset::from_millis(-300)]);

    open_with(&rig, cx, movie());
    assert_eq!(delay(&rig, cx), Offset::default(), "new media starts without one");
}

#[gpui::test]
fn remembered_delays_come_back_unless_set_meanwhile(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    open_with(&rig, cx, movie());
    assert!(set_delay(&rig, cx, 100));
    let media = Media::Url("https://mock/clip".into());
    let saved =
        vec![(TrackId(1), Offset::from_millis(-250)), (TrackId(2), Offset::from_millis(900))];
    rig.run(cx, |player, cx| player.restore_audio_delays(&media, saved, cx));
    assert_eq!(delay(&rig, cx), Offset::from_millis(100), "set meanwhile, so kept");
    hear(&rig, cx, Some(1));
    assert_eq!(delay(&rig, cx), Offset::from_millis(-250));

    let other = Media::Url("https://mock/other".into());
    let late = vec![(TrackId(1), Offset::from_millis(5))];
    rig.run(cx, |player, cx| player.restore_audio_delays(&other, late, cx));
    assert_eq!(delay(&rig, cx), Offset::from_millis(-250), "another media's are ignored");
}

#[gpui::test]
fn a_delay_needs_audio_and_stays_within_ten_minutes(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    open_with(&rig, cx, movie());
    hear(&rig, cx, None);
    assert!(!set_delay(&rig, cx, 500), "no audio plays");
    hear(&rig, cx, Some(2));
    assert!(set_delay(&rig, cx, 900_000));
    assert_eq!(delay(&rig, cx), Offset::from_millis(600_000));
}

#[gpui::test]
fn only_a_changed_stereo_mode_reaches_the_backend(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    rig.mock.take_commands();
    rig.run(cx, |player, cx| player.set_stereo_mode(StereoMode::Mono, cx));
    rig.run(cx, |player, cx| player.set_stereo_mode(StereoMode::Mono, cx));
    assert_eq!(rig.mock.take_commands(), [Command::SetStereoMode(StereoMode::Mono)]);
}

#[gpui::test]
fn only_a_changed_visualizer_reaches_the_backend(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    rig.mock.take_commands();
    let scope = Some(VisualEffect::Scope);
    rig.run(cx, |player, _| player.set_visualizer(scope));
    rig.run(cx, |player, _| player.set_visualizer(scope));
    rig.run(cx, |player, _| player.set_visualizer(None));
    let sent = rig.mock.take_commands();
    assert_eq!(sent, [Command::SetVisualizer(scope), Command::SetVisualizer(None)]);
}

/// Lets the test tone and its margin run out.
fn after_the_tone(cx: &mut TestAppContext) {
    cx.executor().advance_clock(test_tone::LENGTH + Duration::from_secs(1));
    cx.run_until_parked();
}

#[gpui::test]
fn the_test_pauses_the_media_until_the_tone_is_over(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    rig.open(cx);
    rig.mock.take_commands();
    rig.run(cx, Player::play_test_tone);
    assert_eq!(rig.mock.take_commands(), [Command::Pause, Command::PlayTestTone]);
    assert_eq!(rig.state(cx), PlaybackState::Paused);
    after_the_tone(cx);
    assert_eq!(rig.state(cx), PlaybackState::Playing);
}

#[gpui::test]
fn media_the_user_paused_stays_paused_after_the_test(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    rig.open(cx);
    rig.run(cx, Player::pause);
    rig.run(cx, Player::play_test_tone);
    after_the_tone(cx);
    assert_eq!(rig.state(cx), PlaybackState::Paused, "it was paused before");

    rig.run(cx, Player::play);
    rig.run(cx, Player::play_test_tone);
    rig.run(cx, Player::play);
    rig.run(cx, Player::pause);
    after_the_tone(cx);
    assert_eq!(rig.state(cx), PlaybackState::Paused, "the user took over during the test");
}
