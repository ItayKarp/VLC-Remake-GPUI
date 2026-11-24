//! A smoke test of the statistics (S8.13.2) against a real libVLC and the S1.1 test media,
//! ignored by default like [`super::tests`].

use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use super::LibVlcBackend;
use crate::app::media::Media;
use crate::engine::audio::AudioConfig;
use crate::engine::backend::{self, Backend, BackendEvent, EventReceiver};
use crate::engine::shutdown::{SHUTDOWN_TIMEOUT, Shutdown, shut_down};
use crate::engine::state::PlaybackState;
use crate::engine::statistics::Statistics;
use crate::engine::subtitle_style::SubtitleStyle;
use crate::engine::values::{Generation, Start, Volume};

/// How long any one expected event may take.
const PATIENCE: Duration = Duration::from_secs(10);
/// How long the media plays before its counts are read.
const PLAYED: Duration = Duration::from_millis(1500);

/// Waits for the first event `pick` takes; panics after [`PATIENCE`].
fn wait_for<T>(receiver: &mut EventReceiver, pick: impl Fn(BackendEvent) -> Option<T>) -> T {
    let deadline = Instant::now() + PATIENCE;
    while Instant::now() < deadline {
        match receiver.try_recv() {
            Ok(tagged) => {
                if let Some(found) = pick(tagged.event) {
                    return found;
                }
            }
            Err(_) => thread::sleep(Duration::from_millis(5)),
        }
    }
    panic!("timed out");
}

#[test]
#[ignore = "needs libVLC and the S1.1 test media"]
fn playing_video_counts_data_and_pictures() {
    // SAFETY: these tests run one at a time (`--test-threads 1`).
    unsafe { super::runtime::prepare_environment() };
    let (events, mut receiver) = backend::channel();
    let mut backend = Box::new(
        LibVlcBackend::new(events, None, AudioConfig::default(), &SubtitleStyle::default())
            .expect("libVLC"),
    );
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/spike-media");
    backend.set_volume(Volume::new(0.));
    backend.set_mute(true);
    let start = Start { paused: false, ..Start::default() };
    backend.open(&Media::File(path.join("1080i-motion.ts")), start, Generation::default().next());
    wait_for(&mut receiver, |event| {
        (event == BackendEvent::State(PlaybackState::Playing)).then_some(())
    });
    thread::sleep(PLAYED);
    backend.read_statistics();
    let read: Statistics = wait_for(&mut receiver, |event| match event {
        BackendEvent::Statistics(statistics) => Some(statistics),
        _ => None,
    });
    assert!(read.stream_rate.0 > 0.0, "{read:?}");
    assert!(read.decoded_pictures > 0, "{read:?}");
    assert_eq!(shut_down(backend, SHUTDOWN_TIMEOUT), Shutdown::Clean);
}
