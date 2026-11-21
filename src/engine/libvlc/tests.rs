//! Smoke tests against a real libVLC and the S1.1 test media. They are ignored by default
//! (CI has neither); run them with
//!
//! ```text
//! examples/video_spike/make_test_media.sh
//! cargo test engine::libvlc -- --ignored --test-threads 1
//! ```

use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use super::LibVlcBackend;
use crate::app::media::Media;
use crate::engine::backend::{self, Backend, BackendEvent, EventReceiver, EventSender};
use crate::engine::media_info::VideoSize;
use crate::engine::shutdown::{SHUTDOWN_TIMEOUT, Shutdown, shut_down};
use crate::engine::state::{ErrorKind, PlaybackState};
use crate::engine::values::{BackendSeek, Generation, StepDirection};

/// How long any one expected event may take.
const PATIENCE: Duration = Duration::from_secs(10);

/// A libVLC backend and the events it sent, for one test.
struct Session {
    backend: Box<LibVlcBackend>,
    receiver: EventReceiver,
    generation: Generation,
    seen: Vec<BackendEvent>,
}

impl Session {
    fn open(name: &str) -> Self {
        let (events, receiver) = backend::channel();
        let backend = Box::new(new_backend(events));
        let mut session =
            Self { backend, receiver, generation: Generation::default(), seen: Vec::new() };
        session.open_again(name);
        session
    }

    fn open_again(&mut self, name: &str) {
        self.generation = self.generation.next();
        self.backend.open(&Media::File(media(name)), self.generation);
    }

    /// Waits for the first event matching `wanted` and returns it; panics after [`PATIENCE`].
    fn wait_for(&mut self, wanted: impl Fn(&BackendEvent) -> bool) -> BackendEvent {
        let deadline = Instant::now() + PATIENCE;
        while Instant::now() < deadline {
            let Ok(tagged) = self.receiver.try_recv() else {
                thread::sleep(Duration::from_millis(5));
                continue;
            };
            if tagged.generation != self.generation {
                continue;
            }
            self.seen.push(tagged.event.clone());
            if wanted(&tagged.event) {
                return tagged.event;
            }
        }
        panic!("timed out; saw {:#?}", self.seen);
    }

    /// Like [`Self::wait_for`], but an event seen earlier counts too (for media facts, whose
    /// order libVLC doesn't fix).
    fn has_seen(&mut self, wanted: impl Fn(&BackendEvent) -> bool) -> BackendEvent {
        match self.seen.iter().find(|event| wanted(event)) {
            Some(event) => event.clone(),
            None => self.wait_for(wanted),
        }
    }

    fn wait_for_state(&mut self, state: PlaybackState) {
        self.wait_for(|event| *event == BackendEvent::State(state));
    }

    fn finish(self) {
        assert_eq!(shut_down(self.backend, SHUTDOWN_TIMEOUT), Shutdown::Clean);
    }
}

fn new_backend(events: EventSender) -> LibVlcBackend {
    // SAFETY: these tests are run one at a time (`--test-threads 1`, as the module comment
    // says), so no other thread reads the environment meanwhile.
    unsafe { super::runtime::prepare_environment() };
    LibVlcBackend::new(events).expect("libVLC loads")
}

fn media(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/spike-media").join(name)
}

#[test]
#[ignore = "needs libVLC and the S1.1 test media"]
fn a_video_file_plays_seeks_pauses_steps_and_stops() {
    let mut session = Session::open("1080p60-h264.mp4");
    session.wait_for_state(PlaybackState::Playing);
    let duration = session.has_seen(|event| matches!(event, BackendEvent::Duration(Some(_))));
    assert_eq!(duration, BackendEvent::Duration(Some(Duration::from_secs(70))));
    let size = VideoSize { width: 1920, height: 1080 };
    session.has_seen(|event| *event == BackendEvent::VideoSize(Some(size)));

    session.backend.seek(BackendSeek::Time(Duration::from_secs(30)));
    session.wait_for(|event| *event == BackendEvent::SeekCompleted);
    let position = session.wait_for(|event| matches!(event, BackendEvent::Position(_)));
    let BackendEvent::Position(position) = position else { unreachable!() };
    assert!(position >= Duration::from_secs(29), "{position:?}");

    session.backend.pause();
    session.wait_for_state(PlaybackState::Paused);
    session.backend.frame_step(StepDirection::Forward);
    session.wait_for(|event| *event == BackendEvent::FrameStepped);
    session.backend.stop();
    session.wait_for_state(PlaybackState::Stopped);
    session.finish();
}

#[test]
#[ignore = "needs libVLC and the S1.1 test media"]
fn audio_only_media_has_tracks_but_no_video_size() {
    let mut session = Session::open("audio-only.flac");
    session.wait_for_state(PlaybackState::Playing);
    let tracks = session
        .has_seen(|event| matches!(event, BackendEvent::Tracks(tracks) if tracks.audio.is_some()));
    let BackendEvent::Tracks(tracks) = tracks else { unreachable!() };
    assert!(tracks.audio.is_some(), "{tracks:?}");
    assert_eq!(tracks.video, None);
    session.has_seen(|event| *event == BackendEvent::VideoSize(None));
    session.finish();
}

#[test]
#[ignore = "needs libVLC and the S1.1 test media"]
fn seeking_to_the_end_ends_and_reopening_plays_again() {
    let mut session = Session::open("phone-vfr-rot90.mp4");
    session.wait_for_state(PlaybackState::Playing);
    session.backend.seek(BackendSeek::Fraction(1.0));
    session.wait_for_state(PlaybackState::Ended);
    session.open_again("dvd-anamorphic.mpg");
    session.wait_for_state(PlaybackState::Playing);
    session.finish();
}

#[test]
#[ignore = "needs libVLC"]
fn a_file_that_is_not_media_fails() {
    let dir = std::env::temp_dir().join(format!("rvlc-libvlc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let garbage = dir.join("garbage.mp4");
    std::fs::write(&garbage, vec![0x5a_u8; 64 * 1024]).unwrap();
    let (events, receiver) = backend::channel();
    let backend = Box::new(new_backend(events));
    let mut session =
        Session { backend, receiver, generation: Generation::default(), seen: vec![] };
    session.generation = session.generation.next();
    session.backend.open(&Media::File(garbage), session.generation);
    let failed =
        session.wait_for(|event| matches!(event, BackendEvent::State(PlaybackState::Error(_))));
    let expected = [ErrorKind::OpenFailed, ErrorKind::NoPlayableStreams, ErrorKind::Corrupt];
    assert!(expected.iter().any(|kind| failed == BackendEvent::State(PlaybackState::Error(*kind))));
    session.finish();
}
