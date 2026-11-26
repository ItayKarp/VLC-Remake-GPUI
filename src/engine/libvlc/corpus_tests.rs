//! libVLC over the test media corpus (S9.8.1): every media file opens or fails as its
//! manifest says, with the length it should have, and never hangs. Ignored by default; make
//! the corpus with `tools/corpus/make.sh`, then run
//! `cargo test --features runtime_shaders corpus -- --ignored --test-threads 1`. Files open
//! paused and are muted before they play, so the run is silent.

use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use super::LibVlcBackend;
use crate::app::media::Media;
use crate::corpus::{Corpus, CorpusFile, Expect, Kind, assert_none_failed};
use crate::engine::audio::AudioConfig;
use crate::engine::backend::{self, Backend, BackendEvent, EventReceiver};
use crate::engine::shutdown::{SHUTDOWN_TIMEOUT, Shutdown, shut_down};
use crate::engine::state::PlaybackState;
use crate::engine::subtitle_style::SubtitleStyle;
use crate::engine::values::{Generation, Start, Volume};

/// How long a file may take to open, and then to play or fail: the 8K video is slow to start.
const PATIENCE: Duration = Duration::from_secs(20);
/// How far a file must play to count as playing.
const PLAYED: Duration = Duration::from_millis(500);
/// How far a reported length may be from the manifest's.
const LENGTH_TOLERANCE: Duration = Duration::from_secs(1);

/// How an open ended.
#[derive(Debug, PartialEq, Eq)]
enum Settled {
    /// It played past [`PLAYED`].
    Played,
    /// It ended or failed first.
    Failed(PlaybackState),
    /// Neither, within [`PATIENCE`].
    Hung,
}

/// How an open went: how it settled and the length libVLC reported meanwhile.
#[derive(Debug)]
struct Outcome {
    settled: Settled,
    length: Option<Duration>,
}

/// A libVLC backend opening corpus files one after another.
struct Run {
    backend: Box<LibVlcBackend>,
    receiver: EventReceiver,
    generation: Generation,
    length: Option<Duration>,
}

impl Run {
    fn new() -> Self {
        // SAFETY: these tests run one at a time (`--test-threads 1`).
        unsafe { super::runtime::prepare_environment() };
        let (events, receiver) = backend::channel();
        let backend = Box::new(
            LibVlcBackend::new(events, None, AudioConfig::default(), &SubtitleStyle::default())
                .expect("libVLC"),
        );
        Self { backend, receiver, generation: Generation::default(), length: None }
    }

    /// Opens `path` paused, mutes it, plays it and sees how it settles.
    fn try_file(&mut self, path: &Path) -> Outcome {
        self.generation = self.generation.next();
        self.length = None;
        let start = Start { paused: true, ..Start::default() };
        self.backend.open(&Media::File(path.to_path_buf()), start, self.generation);
        let settled =
            match self.wait(|event| matches!(event, BackendEvent::State(PlaybackState::Paused))) {
                Settled::Played => {
                    self.backend.set_volume(Volume::new(0.));
                    self.backend.set_mute(true);
                    self.backend.play();
                    self.wait(|event| matches!(event, BackendEvent::Position(at) if *at >= PLAYED))
                }
                settled => settled,
            };
        self.backend.stop();
        Outcome { settled, length: self.length }
    }

    /// Waits for an event of this open that `done` accepts ([`Settled::Played`]), or for it to
    /// end or fail, noting the length on the way.
    fn wait(&mut self, done: impl Fn(&BackendEvent) -> bool) -> Settled {
        let deadline = Instant::now() + PATIENCE;
        while Instant::now() < deadline {
            let Ok(tagged) = self.receiver.try_recv() else {
                thread::sleep(Duration::from_millis(5));
                continue;
            };
            if tagged.generation != self.generation {
                continue;
            }
            match tagged.event {
                event if done(&event) => return Settled::Played,
                BackendEvent::Duration(length) => self.length = length,
                BackendEvent::State(state @ (PlaybackState::Ended | PlaybackState::Error(_))) => {
                    return Settled::Failed(state);
                }
                _ => {}
            }
        }
        Settled::Hung
    }

    fn finish(self) {
        assert_eq!(shut_down(self.backend, SHUTDOWN_TIMEOUT), Shutdown::Clean);
    }
}

#[test]
#[ignore = "needs libVLC and the S9.8.1 corpus: tools/corpus/make.sh"]
fn every_media_file_plays_or_fails_as_listed() {
    let corpus = Corpus::load();
    let mut run = Run::new();
    let mut failures = Vec::new();
    for file in corpus.files(Kind::Media) {
        let outcome = run.try_file(&corpus.path(file));
        eprintln!("{}: {outcome:?}", file.path);
        failures.extend(problem(file, &outcome));
    }
    run.finish();
    assert_none_failed("media files", &failures);
}

/// What is wrong with `outcome` for `file`, if anything.
fn problem(file: &CorpusFile, outcome: &Outcome) -> Option<String> {
    let path = &file.path;
    match (file.expect, &outcome.settled) {
        (_, Settled::Hung) => Some(format!("{path}: neither played nor failed in {PATIENCE:?}")),
        (Expect::Plays, Settled::Failed(state)) => {
            Some(format!("{path}: {state:?}, expected to play"))
        }
        (Expect::Fails, Settled::Played) => Some(format!("{path}: played, expected to fail")),
        (Expect::Plays, Settled::Played) => length_problem(file, outcome.length),
        _ => None,
    }
}

/// A length libVLC reported that is too far from the manifest's.
fn length_problem(file: &CorpusFile, length: Option<Duration>) -> Option<String> {
    let wanted = Duration::from_secs(file.seconds?);
    let near = length.is_some_and(|length| length.abs_diff(wanted) <= LENGTH_TOLERANCE);
    (!near).then(|| format!("{}: length {length:?}, expected {wanted:?}", file.path))
}
