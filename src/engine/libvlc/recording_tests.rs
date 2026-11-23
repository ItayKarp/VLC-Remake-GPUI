//! Smoke tests of recording (S8.6) against a real libVLC and the S1.1 test media, ignored by
//! default like [`super::tests`].

use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use super::LibVlcBackend;
use crate::app::media::Media;
use crate::engine::audio::AudioConfig;
use crate::engine::backend::{self, Backend, BackendEvent, EventReceiver};
use crate::engine::recording::Recording;
use crate::engine::shutdown::{SHUTDOWN_TIMEOUT, Shutdown, shut_down};
use crate::engine::state::PlaybackState;
use crate::engine::subtitle_style::SubtitleStyle;
use crate::engine::values::{Generation, Start, Volume};

/// How long any one expected event may take.
const PATIENCE: Duration = Duration::from_secs(10);
/// How long each test records.
const RECORD_FOR: Duration = Duration::from_secs(4);

/// Waits for `wanted`, of any open; panics after [`PATIENCE`].
fn wait_for(receiver: &mut EventReceiver, wanted: &BackendEvent) {
    wait_longer(receiver, wanted, PATIENCE);
}

/// Waits for `wanted`, of any open; panics after `patience`.
fn wait_longer(receiver: &mut EventReceiver, wanted: &BackendEvent, patience: Duration) {
    let deadline = Instant::now() + patience;
    while Instant::now() < deadline {
        match receiver.try_recv() {
            Ok(tagged) if tagged.event == *wanted => return,
            Ok(_) => {}
            Err(_) => thread::sleep(Duration::from_millis(5)),
        }
    }
    panic!("timed out waiting for {wanted:?}");
}

/// A backend playing the test media `name`.
fn playing(name: &str) -> (Box<LibVlcBackend>, EventReceiver) {
    // SAFETY: these tests run one at a time (`--test-threads 1`).
    unsafe { super::runtime::prepare_environment() };
    let (events, mut receiver) = backend::channel();
    let mut backend = Box::new(
        LibVlcBackend::new(events, None, AudioConfig::default(), &SubtitleStyle::default())
            .expect("libVLC"),
    );
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/spike-media").join(name);
    // Opened paused and muted before it plays, so the tests make no sound.
    let start = Start { paused: true, ..Start::default() };
    backend.open(&Media::File(path), start, Generation::default().next());
    wait_for(&mut receiver, &BackendEvent::State(PlaybackState::Paused));
    backend.set_volume(Volume::new(0.));
    backend.set_mute(true);
    backend.play();
    wait_for(&mut receiver, &BackendEvent::State(PlaybackState::Playing));
    (backend, receiver)
}

/// A new empty folder for one test's recordings.
fn empty_folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("rvlc-recording-{name}"));
    _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).expect("a temporary folder");
    folder
}

/// The one recording in `folder`, which isn't empty.
fn the_recording(folder: &Path) -> PathBuf {
    let files: Vec<_> = std::fs::read_dir(folder).expect("the folder").flatten().collect();
    assert_eq!(files.len(), 1, "{files:?}");
    let file = files[0].path();
    let name = file.file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.starts_with("vlc-record-"), "{name}");
    assert!(std::fs::metadata(&file).unwrap().len() > 0, "{name} is empty");
    eprintln!("recorded {name}");
    file
}

/// Records `name` for [`RECORD_FOR`], stops, and returns the file.
fn record(name: &str, folder_name: &str) -> PathBuf {
    let (mut backend, mut receiver) = playing(name);
    let folder = empty_folder(folder_name);
    backend.record(Some(&folder));
    wait_for(&mut receiver, &BackendEvent::Recording(Recording::Started));
    thread::sleep(RECORD_FOR);
    backend.record(None);
    wait_for(&mut receiver, &BackendEvent::Recording(Recording::Stopped));
    assert_eq!(shut_down(backend, SHUTDOWN_TIMEOUT), Shutdown::Clean);
    the_recording(&folder)
}

#[test]
#[ignore = "needs libVLC and the S1.1 test media"]
fn an_mp4_is_remuxed_into_a_file() {
    record("1080p60-h264.mp4", "mp4");
}

#[test]
#[ignore = "needs libVLC and the S1.1 test media"]
fn a_transport_stream_is_recorded_as_it_arrives() {
    record("1080i-h264.ts", "ts");
}

#[test]
#[ignore = "needs libVLC and the S1.1 test media"]
fn a_recording_ends_with_the_media() {
    let (mut backend, mut receiver) = playing("1080p60-h264.mp4");
    let folder = empty_folder("ends");
    backend.record(Some(&folder));
    wait_for(&mut receiver, &BackendEvent::Recording(Recording::Started));
    thread::sleep(RECORD_FOR);
    backend.stop();
    wait_for(&mut receiver, &BackendEvent::Recording(Recording::Stopped));
    assert_eq!(shut_down(backend, SHUTDOWN_TIMEOUT), Shutdown::Clean);
    the_recording(&folder);
}

#[test]
#[ignore = "needs libVLC and the S1.1 test media"]
fn a_recording_ends_when_the_media_does() {
    let (mut backend, mut receiver) = playing("1080i-h264.ts");
    let folder = empty_folder("end-of-media");
    backend.record(Some(&folder));
    wait_for(&mut receiver, &BackendEvent::Recording(Recording::Started));
    // The test media is 10 s long; the recording ends just before playback says it ended.
    let stopped = BackendEvent::Recording(Recording::Stopped);
    wait_longer(&mut receiver, &stopped, 2 * PATIENCE);
    wait_for(&mut receiver, &BackendEvent::State(PlaybackState::Ended));
    assert_eq!(shut_down(backend, SHUTDOWN_TIMEOUT), Shutdown::Clean);
    the_recording(&folder);
}
