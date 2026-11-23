//! A smoke test of HDR tone mapping (S7.5) against a real libVLC and the S1.1 test media,
//! ignored by default like [`super::video_tests`].

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use super::LibVlcBackend;
use super::video_tests::{CountingSink, PATIENCE, PICTURES, wait_until};
use crate::app::media::Media;
use crate::engine::audio::AudioConfig;
use crate::engine::backend::{self, Backend, BackendEvent, EventReceiver};
use crate::engine::hdr::{HdrMode, HdrStatus, Transfer};
use crate::engine::shutdown::{SHUTDOWN_TIMEOUT, Shutdown, shut_down};
use crate::engine::subtitle_style::SubtitleStyle;
use crate::engine::values::{Generation, Start};

/// The mean of the picture's green bytes.
fn mean_green(sink: &CountingSink) -> f64 {
    let buffer = sink.buffer.lock().unwrap();
    let greens: Vec<u64> = buffer.iter().skip(1).step_by(4).map(|&g| u64::from(g)).collect();
    greens.iter().sum::<u64>() as f64 / greens.len().max(1) as f64
}

/// Waits for the backend to report an HDR status that `check` accepts.
fn wait_for_status(receiver: &mut EventReceiver, check: impl Fn(HdrStatus) -> bool) {
    let deadline = Instant::now() + PATIENCE;
    loop {
        while let Ok(tagged) = receiver.try_recv() {
            if let BackendEvent::Hdr(status) = tagged.event
                && check(status)
            {
                return;
            }
        }
        assert!(Instant::now() < deadline, "no such HDR status");
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "needs libVLC and the S1.1 test media"]
fn hdr10_is_found_and_tone_mapped_and_off_shows_it_as_decoded() {
    // SAFETY: these tests run one at a time (`--test-threads 1`).
    unsafe { super::runtime::prepare_environment() };
    let sink = Arc::new(CountingSink::default());
    let (events, mut receiver) = backend::channel();
    let mut backend = Box::new(
        LibVlcBackend::new(
            events,
            Some(sink.clone()),
            AudioConfig::default(),
            &SubtitleStyle::default(),
        )
        .expect("libVLC"),
    );
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/spike-media");
    let generation = Generation::default().next();
    backend.open(&Media::File(path.join("1080p-hdr10.mkv")), Start::default(), generation);
    // Auto: the metadata says PQ with a MaxCLL of 1000 nits, so it is reopened tone-mapped.
    wait_for_status(&mut receiver, |status| {
        let pq = status.source.and_then(|source| source.transfer) == Some(Transfer::Pq);
        pq && status.tone_map.is_some_and(|map| (map.peak - 1000.).abs() < 1.)
    });
    let before = sink.displayed.load(Ordering::Relaxed);
    wait_until("mapped pictures", || sink.displayed.load(Ordering::Relaxed) >= before + PICTURES);
    let mapped = mean_green(&sink);
    backend.set_hdr_mode(HdrMode::Off);
    wait_for_status(&mut receiver, |status| status.tone_map.is_none());
    wait_until("pictures as decoded", || (mean_green(&sink) - mapped).abs() > 10.);
    assert_eq!(shut_down(backend, SHUTDOWN_TIMEOUT), Shutdown::Clean);
}
