//! Smoke tests of casting (S8.11) against a real libVLC, ignored by default like
//! [`super::tests`]. No cast device is needed: they check discovery starts and stops cleanly
//! with whatever the network has.

use std::thread;
use std::time::{Duration, Instant};

use super::LibVlcBackend;
use crate::engine::audio::AudioConfig;
use crate::engine::backend::{self, Backend, BackendEvent, EventReceiver};
use crate::engine::cast::{CastDeviceId, CastSetup};
use crate::engine::shutdown::{SHUTDOWN_TIMEOUT, Shutdown, shut_down};
use crate::engine::subtitle_style::SubtitleStyle;
use crate::settings::CastQuality;

/// How long discovery runs before it is stopped.
const DISCOVER_FOR: Duration = Duration::from_secs(3);
/// How long the empty list may take once discovery stops.
const PATIENCE: Duration = Duration::from_secs(10);

/// The lengths of the device lists reported until `deadline` or an empty one.
fn lists_until(receiver: &mut EventReceiver, deadline: Instant) -> Vec<usize> {
    let mut lists = Vec::new();
    while Instant::now() < deadline && lists.last() != Some(&0) {
        match receiver.try_recv() {
            Ok(tagged) => lists.extend(list_length(tagged.event)),
            Err(_) => thread::sleep(Duration::from_millis(5)),
        }
    }
    lists
}

/// How many devices `event` lists, if it is a device list.
fn list_length(event: BackendEvent) -> Option<usize> {
    match event {
        BackendEvent::CastDevices(devices) => Some(devices.len()),
        _ => None,
    }
}

#[test]
#[ignore = "needs libVLC"]
fn discovery_starts_and_stops_cleanly() {
    let (events, mut receiver) = backend::channel();
    // SAFETY: run one at a time, as `super::tests` says.
    unsafe { super::runtime::prepare_environment() };
    let mut backend = Box::new(
        LibVlcBackend::new(events, None, AudioConfig::default(), &SubtitleStyle::default())
            .expect("libVLC loads"),
    );
    let on = CastSetup { discover: true, quality: CastQuality::Low };
    backend.set_cast_setup(on);
    // Nobody but the test knows this device: it is ignored.
    backend.cast(Some(CastDeviceId(u64::MAX)));
    thread::sleep(DISCOVER_FOR);
    let found = lists_until(&mut receiver, Instant::now());
    eprintln!("device lists while discovering: {found:?}");
    backend.set_cast_setup(CastSetup { discover: false, ..on });
    let lists = lists_until(&mut receiver, Instant::now() + PATIENCE);
    assert_eq!(lists.last(), Some(&0), "stopping empties the list: {lists:?}");
    backend.set_cast_setup(on);
    assert_eq!(shut_down(backend, SHUTDOWN_TIMEOUT), Shutdown::Clean);
}
