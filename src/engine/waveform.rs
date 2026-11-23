//! The waveform seek bar's profile (S7.8.2): a local file's loudness along its length, made on a
//! thread of its own and cached per media identity.
//!
//! - [`profile`] gathers the levels from decoded samples, and [`cache`] keeps finished ones.
//! - [`WaveformJob::start`] looks in the cache, else decodes the file's audio through a libVLC
//!   instance of its own, faster than it plays (`libvlc::waveform`), reporting the profile so far
//!   every [`PROGRESS_EVERY`] so a long file draws progressively. Dropping the job stops it.
//!
//! Network streams and live media have no profile: the seek bar stays plain for them.

pub mod cache;
pub mod profile;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use futures::channel::mpsc;

pub use self::profile::{Peaks, Profile};

/// How often a profile being decoded is reported.
pub const PROGRESS_EVERY: Duration = Duration::from_millis(250);

/// What a [`WaveformJob`] reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WaveformEvent {
    /// The profile so far; more follows.
    Progress(Arc<Peaks>),
    /// The whole profile.
    Done(Arc<Peaks>),
    /// No profile can be made: the file has no audio, or it couldn't be decoded.
    Unavailable,
}

/// The receiving end of a job's reports.
pub type WaveformReceiver = mpsc::UnboundedReceiver<WaveformEvent>;

/// A profile being made for one file; dropping it stops the work.
#[derive(Debug)]
pub struct WaveformJob {
    cancelled: Arc<AtomicBool>,
}

impl WaveformJob {
    /// Starts making the profile of the file at `path`, `length` long, on a thread of its own.
    pub fn start(path: PathBuf, length: Duration) -> (Self, WaveformReceiver) {
        let (sender, receiver) = mpsc::unbounded();
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let started = thread::Builder::new()
            .name("waveform".into())
            .spawn(move || run(&path, length, &flag, &sender));
        if let Err(error) = started {
            tracing::warn!("waveform: couldn't start a thread: {error}");
        }
        (Self { cancelled }, receiver)
    }
}

impl Drop for WaveformJob {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

/// Makes the profile of `path` and reports it through `events`, unless `cancelled` first.
fn run(
    path: &std::path::Path,
    length: Duration,
    cancelled: &AtomicBool,
    events: &mpsc::UnboundedSender<WaveformEvent>,
) {
    let cached = cache::file_for(path);
    if let Some(peaks) = cached.as_deref().and_then(cache::load) {
        let _ = events.unbounded_send(WaveformEvent::Done(Arc::new(peaks)));
        return;
    }
    let report = |profile: &Profile| {
        let peaks = Arc::new(profile.peaks(false));
        let _ = events.unbounded_send(WaveformEvent::Progress(peaks));
    };
    let decoded = super::libvlc::waveform::decode(path, length, cancelled, report);
    if !cancelled.load(Ordering::Relaxed) {
        let _ = events.unbounded_send(finish(path, decoded, cached.as_deref()));
    }
}

/// What a finished decode of `path` says: its profile, cached at `cached`, or none.
fn finish(
    path: &std::path::Path,
    decoded: anyhow::Result<Profile>,
    cached: Option<&std::path::Path>,
) -> WaveformEvent {
    let profile = match decoded {
        Ok(profile) => profile,
        Err(error) => {
            tracing::info!("waveform: no profile for {}: {error:#}", path.display());
            return WaveformEvent::Unavailable;
        }
    };
    let peaks = profile.peaks(true);
    if let Some(file) = cached {
        keep(file, &peaks);
    }
    WaveformEvent::Done(Arc::new(peaks))
}

/// Caches `peaks` at `file`; a failure only costs making them again.
fn keep(file: &std::path::Path, peaks: &Peaks) {
    if let Err(error) = cache::save(file, peaks) {
        tracing::warn!("waveform: couldn't cache the profile: {error}");
    }
}
