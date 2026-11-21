//! Smoke tests of the `vmem` path (S1.4.1) against a real libVLC and the S1.1 test media.
//! Ignored by default, like [`super::tests`]; run them with
//!
//! ```text
//! cargo test engine::libvlc -- --ignored --test-threads 1
//! ```

use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use super::LibVlcBackend;
use crate::app::media::Media;
use crate::engine::audio::AudioConfig;
use crate::engine::backend::{self, Backend, BackendEvent};
use crate::engine::media_info::VideoSize;
use crate::engine::shutdown::{SHUTDOWN_TIMEOUT, Shutdown, shut_down};
use crate::engine::values::Generation;
use crate::engine::video::{Format, MAX_PLANES, VideoSink};

/// How long the first pictures may take.
const PATIENCE: Duration = Duration::from_secs(10);
/// Pictures to receive before a file counts as playing.
const PICTURES: usize = 10;

/// A BGRA sink that records the size it agreed on and counts pictures.
#[derive(Default)]
struct CountingSink {
    size: Mutex<Option<(u32, u32)>>,
    buffer: Mutex<Vec<u8>>,
    displayed: AtomicUsize,
}

impl VideoSink for CountingSink {
    fn setup(&self, format: &mut Format) -> bool {
        let (width, height) = format.chosen_size();
        *format = Format { chroma: *b"BGRA", width, height, ..*format };
        format.pitches[0] = width * 4;
        format.lines[0] = height;
        *self.buffer.lock().unwrap() = vec![0; (width * height * 4) as usize];
        *self.size.lock().unwrap() = Some((width, height));
        true
    }

    fn lock(&self, planes: &mut [*mut c_void; MAX_PLANES]) {
        planes[0] = self.buffer.lock().unwrap().as_mut_ptr().cast();
    }

    fn unlock(&self) {}

    fn display(&self) {
        self.displayed.fetch_add(1, Ordering::Relaxed);
    }

    fn cleanup(&self) {}
}

/// Plays `name` until [`PICTURES`] arrive, and returns the size the sink was set up with.
fn shown_size(name: &str) -> (u32, u32) {
    // SAFETY: these tests run one at a time (`--test-threads 1`), so no other thread reads
    // the environment meanwhile.
    unsafe { super::runtime::prepare_environment() };
    let sink = Arc::new(CountingSink::default());
    let (events, _receiver) = backend::channel();
    let mut backend = Box::new(
        LibVlcBackend::new(events, Some(sink.clone()), AudioConfig::default()).expect("libVLC"),
    );
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/spike-media").join(name);
    backend.open(&Media::File(path), Generation::default().next());
    let deadline = Instant::now() + PATIENCE;
    while sink.displayed.load(Ordering::Relaxed) < PICTURES {
        assert!(Instant::now() < deadline, "{name}: no pictures");
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(shut_down(backend, SHUTDOWN_TIMEOUT), Shutdown::Clean);
    let size = *sink.size.lock().unwrap();
    size.expect("set up before the first picture")
}

#[test]
#[ignore = "needs libVLC and the S1.1 test media"]
fn coded_padding_is_cropped_to_the_visible_size() {
    // H.264 1080p is coded as 1920×1088; the parse tells the sink to ask for 1080 rows.
    assert_eq!(shown_size("1080p60-h264.mp4"), (1920, 1080));
}

#[test]
#[ignore = "needs libVLC and the S1.1 test media"]
fn anamorphic_and_odd_sizes_arrive_in_square_pixels() {
    assert_eq!(shown_size("odd-1919x1079.mkv"), (1919, 1079));
}

#[test]
#[ignore = "needs libVLC and the S1.1 test media"]
fn rotated_phone_video_plays_upright() {
    // Without software decoding libVLC 3 shows no picture at all for a rotated track.
    let (width, height) = shown_size("phone-vfr-rot90.mp4");
    assert!(height > width, "{width}x{height} is not portrait");
}

#[test]
#[ignore = "needs libVLC and the S1.1 test media"]
fn a_dvd_reveals_its_wide_shape_soon_after_playback_starts() {
    // SAFETY: as in `shown_size`.
    unsafe { super::runtime::prepare_environment() };
    let sink = Arc::new(CountingSink::default());
    let (events, mut receiver) = backend::channel();
    let mut backend =
        Box::new(LibVlcBackend::new(events, Some(sink), AudioConfig::default()).expect("libVLC"));
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/spike-media");
    backend.open(&Media::File(path.join("dvd-anamorphic.mpg")), Generation::default().next());
    let wide = BackendEvent::VideoSize(Some(VideoSize { width: 853, height: 480 }));
    let deadline = Instant::now() + PATIENCE;
    while receiver.try_recv().map(|tagged| tagged.event) != Ok(wide.clone()) {
        assert!(Instant::now() < deadline, "the 16:9 shape never arrived");
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(shut_down(backend, SHUTDOWN_TIMEOUT), Shutdown::Clean);
}
