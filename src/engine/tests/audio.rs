//! Player audio tests (S1.5): the volume range, mute, hot-plug fallback and pausing for
//! another app's audio; and the live effects beside them, the picture's adjustments (S7.1).

use gpui::TestAppContext;

use super::{Rig, secs};
use crate::app::media::Media;
use crate::engine::audio::{AudioDevice, AudioPolicy, OutputDevice};
use crate::engine::backend::{BackendEvent, Command};
use crate::engine::mock::MockMedia;
use crate::engine::player::{Player, PlayerEvent};
use crate::engine::state::PlaybackState;
use crate::engine::values::{Volume, VolumeChange};

fn device(id: &str) -> AudioDevice {
    AudioDevice { id: id.into(), name: format!("Device {id}") }
}

fn headphones() -> OutputDevice {
    OutputDevice::Device("2".into())
}

fn volume(rig: &Rig, cx: &mut TestAppContext) -> Volume {
    rig.player.read_with(cx, |player, _| player.volume())
}

#[gpui::test]
fn the_volume_stays_under_the_maximum(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    rig.run(cx, |player, cx| player.set_volume(Volume::from_percent(290), cx));
    assert_eq!(volume(&rig, cx), Volume::from_percent(125));
    rig.run(cx, |player, cx| player.set_max_volume(Volume::from_percent(300), cx));
    rig.run(cx, |player, cx| player.set_volume(Volume::from_percent(290), cx));
    assert_eq!(volume(&rig, cx), Volume::from_percent(290));
    rig.run(cx, |player, cx| player.set_max_volume(Volume::from_percent(150), cx));
    assert_eq!(volume(&rig, cx), Volume::from_percent(150));
    rig.run(cx, |player, cx| player.set_max_volume(Volume::from_percent(300), cx));
    let step = Volume::from_percent(5);
    rig.run(cx, |player, cx| player.step_volume(VolumeChange::Up, step, cx));
    assert_eq!(volume(&rig, cx), Volume::from_percent(155));
    let sent = rig.mock.take_commands();
    assert_eq!(sent.last(), Some(&Command::SetVolume(Volume::from_percent(155))));
}

#[gpui::test]
fn changing_the_volume_unmutes(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    rig.run(cx, Player::toggle_mute);
    assert!(rig.player.read_with(cx, |player, _| player.is_muted()));
    let step = Volume::from_percent(5);
    rig.run(cx, |player, cx| player.step_volume(VolumeChange::Down, step, cx));
    assert!(!rig.player.read_with(cx, |player, _| player.is_muted()));
    assert_eq!(rig.mock.take_commands().last(), Some(&Command::SetMute(false)));
    assert_eq!(rig.count(&PlayerEvent::VolumeChanged), 2);
}

#[gpui::test]
fn an_unplugged_device_falls_back_pauses_and_returns(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    rig.emit(cx, BackendEvent::AudioDevices(vec![device("1"), device("2")]));
    rig.run(cx, |player, cx| player.set_audio_device(headphones(), cx));
    rig.open(cx);
    rig.mock.take_commands();
    rig.emit(cx, BackendEvent::AudioDevices(vec![device("1")]));
    let fallback = [Command::SetAudioDevice(OutputDevice::SystemDefault), Command::Pause];
    assert_eq!(rig.mock.take_commands(), fallback);
    assert_eq!(rig.state(cx), PlaybackState::Paused);
    assert_eq!(rig.count(&PlayerEvent::AudioDeviceLost(device("2"))), 1);
    rig.emit(cx, BackendEvent::AudioDevices(vec![device("1"), device("2")]));
    assert_eq!(rig.mock.take_commands(), [Command::SetAudioDevice(headphones())]);
}

#[gpui::test]
fn device_lists_apply_whatever_open_they_carry(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    rig.open(cx);
    let first = rig.mock.generation();
    rig.open(cx);
    rig.mock.emit_for(first, BackendEvent::AudioDevices(Vec::new()));
    cx.run_until_parked();
    assert!(rig.player.read_with(cx, |player, _| player.has_no_audio_device()));
}

#[gpui::test]
fn another_apps_audio_pauses_until_it_ends(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    rig.open(cx);
    rig.emit(cx, BackendEvent::Corked(true));
    assert_eq!(rig.state(cx), PlaybackState::Paused);
    rig.emit(cx, BackendEvent::Corked(false));
    assert_eq!(rig.state(cx), PlaybackState::Playing);
}

#[gpui::test]
fn a_user_pause_during_a_call_is_kept(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    rig.open(cx);
    rig.emit(cx, BackendEvent::Corked(true));
    rig.run(cx, Player::pause);
    rig.emit(cx, BackendEvent::Corked(false));
    assert_eq!(rig.state(cx), PlaybackState::Paused);
}

#[gpui::test]
fn the_policy_can_keep_playing(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    let policy = AudioPolicy { pause_on_device_loss: false, pause_when_corked: false };
    rig.run(cx, |player, _| player.set_audio_policy(policy));
    rig.emit(cx, BackendEvent::AudioDevices(vec![device("1")]));
    rig.run(cx, |player, cx| player.open(Media::Url("mock://clip".into()), cx));
    rig.emit(cx, BackendEvent::Corked(true));
    rig.emit(cx, BackendEvent::AudioDevices(Vec::new()));
    assert_eq!(rig.state(cx), PlaybackState::Playing);
    assert_eq!(rig.count(&PlayerEvent::AudioDeviceLost(device("1"))), 1);
    assert_eq!(rig.player.read_with(cx, |player, _| player.duration()), Some(secs(60)));
}

#[gpui::test]
fn only_a_changed_picture_reaches_the_backend(cx: &mut TestAppContext) {
    use crate::engine::media_options::Picture;

    let rig = Rig::new(cx, MockMedia::default());
    rig.mock.take_commands();
    let bright = Some(Picture { brightness: 1.5, ..Picture::NEUTRAL });
    rig.run(cx, |player, _| player.set_picture(bright));
    rig.run(cx, |player, _| player.set_picture(bright));
    rig.run(cx, |player, _| player.set_picture(None));
    assert_eq!(rig.mock.take_commands(), [Command::SetPicture(bright), Command::SetPicture(None)]);
    assert_eq!(rig.player.read_with(cx, |player, _| player.picture()), None);
}

#[gpui::test]
fn only_changed_video_filters_reach_the_backend(cx: &mut TestAppContext) {
    use crate::engine::media_options::VideoFilters;
    use crate::engine::transform::{Rotation, Transform};

    let rig = Rig::new(cx, MockMedia::default());
    rig.mock.take_commands();
    let turned = VideoFilters {
        transform: Transform { rotation: Rotation::Quarter, ..Transform::NONE },
        ..VideoFilters::default()
    };
    rig.run(cx, |player, _| player.set_video_filters(VideoFilters::default()));
    rig.run(cx, |player, _| player.set_video_filters(turned));
    rig.run(cx, |player, _| player.set_video_filters(turned));
    assert_eq!(rig.mock.take_commands(), [Command::SetVideoFilters(turned)]);
}
