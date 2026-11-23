//! Smoke tests of video that opens paused, showing its picture (`hold.rs`): a paused video
//! reopened for new filters (S7.2, S7.3) and media opened paused (S3.4), against a real libVLC
//! and the S1.1 test media, ignored by default like [`super::video_tests`].

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use super::LibVlcBackend;
use super::video_tests::{CountingSink, PICTURES, wait_until};
use crate::app::media::Media;
use crate::engine::audio::AudioConfig;
use crate::engine::backend::{self, Backend, BackendEvent};
use crate::engine::media_options::Picture;
use crate::engine::shutdown::{SHUTDOWN_TIMEOUT, Shutdown, shut_down};
use crate::engine::state::PlaybackState;
use crate::engine::subtitle_style::SubtitleStyle;
use crate::engine::values::{Generation, Start};

/// The mean of the picture's bytes: 0 while nothing was drawn into it.
fn level(sink: &CountingSink) -> f64 {
    let buffer = sink.buffer.lock().unwrap();
    let sum: u64 = buffer.iter().map(|&byte| u64::from(byte)).sum();
    sum as f64 / buffer.len().max(1) as f64
}

#[test]
#[ignore = "needs libVLC and the S1.1 test media"]
fn paused_video_reopened_for_a_filter_shows_its_picture_and_stays_paused() {
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
    let media = Media::File(path.join("1080p60-h264.mp4"));
    backend.open(&media, Start::default(), Generation::default().next());
    wait_until("pictures", || sink.displayed.load(Ordering::Relaxed) >= PICTURES);
    backend.pause();
    thread::sleep(Duration::from_millis(500));
    while receiver.try_recv().is_ok() {}
    // Turning grain on reopens the video in software with a new output.
    backend.set_picture(Some(Picture { grain: 5., ..Picture::NEUTRAL }));
    wait_until("the picture", || level(&sink) > 40.);
    // Paused, it may still draw the picture again through its filters, but time stands still.
    thread::sleep(Duration::from_secs(3));
    let mut positions = Vec::new();
    while let Ok(tagged) = receiver.try_recv() {
        match tagged.event {
            BackendEvent::Position(position) => positions.push(position),
            BackendEvent::State(state) => {
                assert_ne!(state, PlaybackState::Playing, "the player saw it play");
            }
            BackendEvent::AudioDevices(devices) => {
                assert!(!devices.is_empty(), "the devices seemed to go away");
            }
            _ => {}
        }
    }
    let (first, last) = (positions.iter().min(), positions.iter().max());
    let moved = last.zip(first).map_or(Duration::ZERO, |(last, first)| last.saturating_sub(*first));
    assert!(moved < Duration::from_millis(300), "it went on playing for {moved:?}");
    assert_eq!(shut_down(backend, SHUTDOWN_TIMEOUT), Shutdown::Clean);
}

#[test]
#[ignore = "needs libVLC and the S1.1 test media"]
fn video_opened_paused_shows_its_picture_and_stays_paused() {
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
    let media = Media::File(path.join("1080p60-h264.mp4"));
    let start = Start { at: Duration::from_secs(5), paused: true };
    backend.open(&media, start, Generation::default().next());
    wait_until("the picture", || level(&sink) > 40.);
    thread::sleep(Duration::from_secs(1));
    let mut positions = Vec::new();
    while let Ok(tagged) = receiver.try_recv() {
        match tagged.event {
            BackendEvent::Position(position) => positions.push(position),
            BackendEvent::State(state) => {
                assert_ne!(state, PlaybackState::Playing, "the player saw it play");
            }
            _ => {}
        }
    }
    let last = positions.iter().max().copied().unwrap_or_default();
    assert!(last < Duration::from_millis(5300), "it went on playing to {last:?}");
    assert_eq!(shut_down(backend, SHUTDOWN_TIMEOUT), Shutdown::Clean);
}
