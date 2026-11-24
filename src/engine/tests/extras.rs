//! Player tests for the playback extras (S8.13): the sleep timer's fade, the A-B loop and the
//! statistics.

use gpui::TestAppContext;

use super::{Rig, secs};
use crate::engine::ab_loop::AbLoop;
use crate::engine::backend::{BackendEvent, Command};
use crate::engine::mock::MockMedia;
use crate::engine::player::Player;
use crate::engine::state::PlaybackState;
use crate::engine::statistics::Statistics;
use crate::engine::values::{BackendSeek, Volume};

/// The volume commands the backend was given.
fn volumes(rig: &Rig) -> Vec<Volume> {
    let commands = rig.mock.take_commands().into_iter();
    commands
        .filter_map(|command| match command {
            Command::SetVolume(volume) => Some(volume),
            _ => None,
        })
        .collect()
}

/// A manual mock playing a seekable minute, its commands so far forgotten.
fn playing(cx: &mut TestAppContext) -> Rig {
    let rig = Rig::new(cx, MockMedia::manual());
    rig.open(cx);
    rig.emit(cx, BackendEvent::State(PlaybackState::Playing));
    rig.emit(cx, BackendEvent::Duration(Some(secs(60))));
    rig.emit(cx, BackendEvent::Seekable(true));
    rig.mock.take_commands();
    rig
}

fn mark_at(rig: &Rig, cx: &mut TestAppContext, at: u64) -> Option<AbLoop> {
    rig.emit(cx, BackendEvent::Position(secs(at)));
    rig.player.update(cx, Player::mark_ab_loop)
}

#[gpui::test]
fn the_fade_lowers_what_is_heard_but_not_the_volume_set(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    rig.run(cx, |player, cx| player.set_volume(Volume::new(0.8), cx));
    rig.mock.take_commands();
    rig.run(cx, |player, cx| player.set_fade(0.5, cx));
    rig.run(cx, |player, cx| player.set_volume(Volume::new(0.6), cx));
    assert_eq!(volumes(&rig), [Volume::new(0.4), Volume::new(0.3)]);
    assert_eq!(rig.player.read_with(cx, |player, _| player.volume()), Volume::new(0.6));
    rig.run(cx, |player, cx| player.set_fade(1.0, cx));
    assert_eq!(volumes(&rig), [Volume::new(0.6)]);
}

#[gpui::test]
fn an_a_b_loop_goes_back_to_a_from_b(cx: &mut TestAppContext) {
    let rig = playing(cx);
    assert_eq!(mark_at(&rig, cx, 10), Some(AbLoop::From(secs(10))));
    assert_eq!(mark_at(&rig, cx, 20), Some(AbLoop::Between { a: secs(10), b: secs(20) }));
    rig.mock.take_commands();
    rig.emit(cx, BackendEvent::Position(secs(21)));
    assert_eq!(rig.mock.take_commands(), [Command::Seek(BackendSeek::Time(secs(10)))]);
    assert_eq!(mark_at(&rig, cx, 12), Some(AbLoop::Off));
}

#[gpui::test]
fn a_loop_ends_with_its_media_and_needs_seeking(cx: &mut TestAppContext) {
    let rig = playing(cx);
    mark_at(&rig, cx, 10);
    rig.open(cx);
    assert_eq!(rig.player.read_with(cx, |player, _| player.ab_loop()), AbLoop::Off);
    rig.emit(cx, BackendEvent::State(PlaybackState::Playing));
    rig.emit(cx, BackendEvent::Seekable(false));
    assert_eq!(mark_at(&rig, cx, 10), None);
}

#[gpui::test]
fn statistics_are_read_while_media_is_open_and_forgotten_after(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    rig.run(cx, |player, _| player.read_statistics());
    assert_eq!(rig.mock.take_commands(), []);
    rig.open(cx);
    rig.run(cx, |player, _| player.read_statistics());
    let read = rig.player.read_with(cx, |player, _| player.statistics().copied());
    assert_eq!(read, Some(Statistics::default()));
    rig.open(cx);
    assert_eq!(rig.player.read_with(cx, |player, _| player.statistics().copied()), None);
}
