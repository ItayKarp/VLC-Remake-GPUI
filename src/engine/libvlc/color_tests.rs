//! A smoke test of the color filters (S7.4) against a real libVLC and the S1.1 test media,
//! ignored by default like [`super::video_tests`].

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use super::LibVlcBackend;
use super::video_tests::{CountingSink, PICTURES, wait_until};
use crate::app::media::Media;
use crate::engine::audio::AudioConfig;
use crate::engine::backend::{self, Backend};
use crate::engine::colors::Colors;
use crate::engine::shutdown::{SHUTDOWN_TIMEOUT, Shutdown, shut_down};
use crate::engine::subtitle_style::SubtitleStyle;
use crate::engine::values::{Generation, Start};

/// The mean of the picture's green bytes.
fn mean_green(sink: &CountingSink) -> f64 {
    let buffer = sink.buffer.lock().unwrap();
    let greens: Vec<u64> = buffer.iter().skip(1).step_by(4).map(|&g| u64::from(g)).collect();
    greens.iter().sum::<u64>() as f64 / greens.len().max(1) as f64
}

/// How many different green bytes the picture has.
fn greens(sink: &CountingSink) -> usize {
    let buffer = sink.buffer.lock().unwrap();
    buffer.iter().skip(1).step_by(4).collect::<HashSet<_>>().len()
}

#[test]
#[ignore = "needs libVLC and the S1.1 test media"]
fn negate_goes_on_and_off_and_posterize_changes_live() {
    // SAFETY: these tests run one at a time (`--test-threads 1`).
    unsafe { super::runtime::prepare_environment() };
    let sink = Arc::new(CountingSink::default());
    let (events, _receiver) = backend::channel();
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
    backend.open(
        &Media::File(path.join("1080p60-h264.mp4")),
        Start::default(),
        Generation::default().next(),
    );
    wait_until("pictures", || sink.displayed.load(Ordering::Relaxed) >= PICTURES);
    let plain = mean_green(&sink);
    backend.set_colors(Colors { negate: true, ..Colors::NONE });
    // Turning it on reopens the video in software, as VideoToolbox's frames can't be inverted.
    wait_until("negative", || (mean_green(&sink) - (255. - plain)).abs() < 20.);
    backend.set_colors(Colors { posterize: Some(2), ..Colors::NONE });
    wait_until("two levels", || greens(&sink) <= 4);
    backend.set_colors(Colors { posterize: Some(64), ..Colors::NONE });
    // Live: the same output, its filter given the new levels.
    wait_until("more levels", || greens(&sink) > 8);
    backend.set_colors(Colors::NONE);
    wait_until("as decoded", || (mean_green(&sink) - plain).abs() < 20.);
    assert_eq!(shut_down(backend, SHUTDOWN_TIMEOUT), Shutdown::Clean);
}
