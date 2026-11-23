//! A smoke test of the picture's enhancements (S7.3) against a real libVLC and the S1.1 test
//! media, ignored by default like [`super::video_tests`].

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use super::LibVlcBackend;
use super::video_tests::{CountingSink, PICTURES, wait_until};
use crate::app::media::Media;
use crate::engine::audio::AudioConfig;
use crate::engine::backend::{self, Backend};
use crate::engine::media_options::Picture;
use crate::engine::shutdown::{SHUTDOWN_TIMEOUT, Shutdown, shut_down};
use crate::engine::subtitle_style::SubtitleStyle;
use crate::engine::values::{Generation, Start};

/// How rough the test card's flat top-left bar is: the mean difference between neighbouring
/// pixels' green bytes, about 0 while it is flat.
fn roughness(sink: &CountingSink) -> f64 {
    let Some((width, height)) = *sink.size.lock().unwrap() else { return 0. };
    let buffer = sink.buffer.lock().unwrap();
    let (width, height) = (width as usize, height as usize);
    let (mut sum, mut count) = (0_u64, 0_u64);
    for y in height / 20..height / 8 {
        let row = &buffer[y * width * 4..(y + 1) * width * 4];
        for x in 8..width / 16 {
            sum += u64::from(row[x * 4 + 1].abs_diff(row[(x + 1) * 4 + 1]));
            count += 1;
        }
    }
    sum as f64 / count.max(1) as f64
}

#[test]
#[ignore = "needs libVLC and the S1.1 test media"]
fn film_grain_goes_on_changes_live_and_goes_off() {
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
    let flat = roughness(&sink);
    backend.set_picture(Some(Picture { grain: 10., ..Picture::NEUTRAL }));
    // Turning it on reopens the video in software, as VideoToolbox's frames can't take grain.
    wait_until("grain", || roughness(&sink) > flat + 0.8);
    let grainy = roughness(&sink);
    backend.set_picture(Some(Picture { grain: 1., ..Picture::NEUTRAL }));
    // Live: the same output, its filter given the new variance.
    wait_until("less grain", || roughness(&sink) - flat < (grainy - flat) / 2.);
    backend.set_picture(None);
    wait_until("no grain", || roughness(&sink) < flat + 0.2);
    assert_eq!(shut_down(backend, SHUTDOWN_TIMEOUT), Shutdown::Clean);
}
