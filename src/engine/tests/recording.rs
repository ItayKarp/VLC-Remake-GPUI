//! Player tests for recording (S8.6): the backend is asked once, the player says what became
//! of it, a recording another open ends is still reported, and seeking waits until it ends.

use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui::TestAppContext;

use super::Rig;
use crate::engine::backend::{BackendEvent, Command};
use crate::engine::mock::MockMedia;
use crate::engine::player::PlayerEvent;
use crate::engine::recording::Recording;
use crate::engine::values::SeekTarget;

fn recording(rig: &Rig, cx: &mut TestAppContext) -> bool {
    rig.player.read_with(cx, |player, _| player.recorded().is_some())
}

fn record_commands(rig: &Rig) -> Vec<Option<PathBuf>> {
    let commands = rig.mock.take_commands().into_iter();
    commands
        .filter_map(|command| match command {
            Command::Record(folder) => Some(folder),
            _ => None,
        })
        .collect()
}

#[gpui::test]
fn recording_starts_once_and_stops(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    rig.open(cx);
    let folder = Path::new("/recordings");
    rig.run(cx, |player, _| player.start_recording(folder));
    rig.run(cx, |player, _| player.start_recording(folder));
    assert!(recording(&rig, cx));
    assert_eq!(rig.count(&PlayerEvent::RecordingChanged(Recording::Started)), 1);
    rig.run(cx, |player, _| player.stop_recording());
    assert!(!recording(&rig, cx));
    assert_eq!(rig.count(&PlayerEvent::RecordingChanged(Recording::Stopped)), 1);
    assert_eq!(record_commands(&rig), [Some(folder.to_path_buf()), None]);
}

#[gpui::test]
fn a_recording_ended_by_another_open_is_still_reported(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    rig.open(cx);
    rig.emit(cx, BackendEvent::Recording(Recording::Started));
    let old = rig.mock.generation();
    rig.open(cx);
    rig.mock.emit_for(old, BackendEvent::Recording(Recording::Stopped));
    cx.run_until_parked();
    assert!(!recording(&rig, cx));
    assert_eq!(rig.count(&PlayerEvent::RecordingChanged(Recording::Stopped)), 1);
}

#[gpui::test]
fn a_refusal_leaves_it_not_recording(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    rig.open(cx);
    rig.emit(cx, BackendEvent::Recording(Recording::Refused));
    assert!(!recording(&rig, cx));
    assert_eq!(rig.count(&PlayerEvent::RecordingChanged(Recording::Refused)), 1);
}

#[gpui::test]
fn seeking_is_refused_while_recording(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    rig.open(cx);
    rig.run(cx, |player, _| player.start_recording(Path::new("/recordings")));
    rig.mock.take_commands();
    rig.run(cx, |player, cx| player.seek(SeekTarget::Absolute(Duration::from_secs(30)), cx));
    assert_eq!(rig.count(&PlayerEvent::SeekRefused), 1);
    assert!(!rig.mock.take_commands().iter().any(|command| matches!(command, Command::Seek(_))));
}
