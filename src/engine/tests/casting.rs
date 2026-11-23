//! Player casting tests (S8.11): the devices found, playing on one, and losing it.

use gpui::TestAppContext;

use super::Rig;
use crate::engine::backend::{BackendEvent, Command};
use crate::engine::cast::{CastDevice, CastDeviceId, CastSetup};
use crate::engine::mock::MockMedia;
use crate::engine::player::PlayerEvent;
use crate::settings::CastQuality;

fn device(id: u64) -> CastDevice {
    CastDevice { id: CastDeviceId(id), name: format!("TV {id}"), video: true }
}

fn target(rig: &Rig, cx: &mut TestAppContext) -> Option<CastDeviceId> {
    rig.player.read_with(cx, |player, _| player.cast_target().map(|device| device.id))
}

#[gpui::test]
fn the_setup_reaches_the_backend_once(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    let setup = CastSetup { discover: true, quality: CastQuality::Low };
    rig.run(cx, |player, _| player.set_cast_setup(setup));
    rig.run(cx, |player, _| player.set_cast_setup(setup));
    let sent = rig.mock.take_commands();
    assert_eq!(sent.iter().filter(|command| **command == Command::SetCastSetup(setup)).count(), 1);
}

#[gpui::test]
fn only_a_known_device_is_cast_to(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    rig.run(cx, |player, cx| player.cast_to(Some(CastDeviceId(1)), cx));
    assert_eq!(target(&rig, cx), None);
    rig.emit(cx, BackendEvent::CastDevices(vec![device(1), device(2)]));
    assert_eq!(rig.count(&PlayerEvent::CastDevicesChanged), 1);
    rig.run(cx, |player, cx| player.cast_to(Some(CastDeviceId(2)), cx));
    assert_eq!(target(&rig, cx), Some(CastDeviceId(2)));
    rig.run(cx, |player, cx| player.cast_to(None, cx));
    assert_eq!(target(&rig, cx), None);
    let casts: Vec<_> = rig
        .mock
        .take_commands()
        .into_iter()
        .filter(|command| matches!(command, Command::Cast(_)))
        .collect();
    assert_eq!(casts, [Command::Cast(Some(CastDeviceId(2))), Command::Cast(None)]);
    assert_eq!(rig.count(&PlayerEvent::CastChanged), 2);
}

#[gpui::test]
fn a_lost_device_plays_here_again(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    rig.emit(cx, BackendEvent::CastDevices(vec![device(1)]));
    rig.run(cx, |player, cx| player.cast_to(Some(CastDeviceId(1)), cx));
    rig.emit(cx, BackendEvent::CastLost(device(1)));
    rig.emit(cx, BackendEvent::CastDevices(Vec::new()));
    assert_eq!(target(&rig, cx), None);
    assert_eq!(rig.count(&PlayerEvent::CastLost(device(1))), 1);
}

#[gpui::test]
fn a_device_gone_from_the_list_isnt_the_target(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    rig.emit(cx, BackendEvent::CastDevices(vec![device(1)]));
    rig.run(cx, |player, cx| player.cast_to(Some(CastDeviceId(1)), cx));
    rig.emit(cx, BackendEvent::CastDevices(Vec::new()));
    assert_eq!(target(&rig, cx), None);
    assert_eq!(rig.count(&PlayerEvent::CastChanged), 2);
}
