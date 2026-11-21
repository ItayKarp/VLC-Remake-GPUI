//! The last session (S3.4): the queue, its current item and where that was, and Repeat and
//! Shuffle, kept in `session.toml` next to the local state so the next launch can bring them
//! back, paused, instead of starting empty.
//!
//! - [`SessionStore`]: the GPUI global. [`init`] reads what the last run left and starts the
//!   writer thread ([`writer`]), so saving never waits on disk; quitting waits for the last write.
//! - [`Snapshot`]: what is saved, in the file's own form ([`file`]).
//! - [`restore`]: what the next launch does with it, including the crash guard.
//! - [`cadence`]: when a change is worth writing.
//!
//! The snapshot says whether something was playing. It is written at once when playback starts
//! and cleared by every pause, stop and quit, so a launch that finds it set knows the last run
//! ended while that item played: it may have crashed the decoder, so it isn't loaded again.

pub mod cadence;
mod file;
pub mod restore;
mod writer;

use std::sync::mpsc::Sender;
use std::time::Duration;

use futures::channel::oneshot;
use gpui::{App, BorrowAppContext as _, Global};

use self::writer::Job;
use crate::app::media::Media;
use crate::queue::Repeat;
use crate::settings::Locations;

/// What a session leaves for the next launch.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    /// The queue, in order.
    pub media: Vec<Media>,
    /// The current item's index in [`Self::media`].
    pub current: Option<usize>,
    /// How far into the current item playback was.
    pub position: Duration,
    /// The repeat mode.
    pub repeat: Repeat,
    /// Whether Shuffle was on.
    pub shuffle: bool,
    /// Whether the current item was playing (not paused, stopped or finished) when this was
    /// saved.
    pub playing: bool,
}

/// The session service: what the last run left, until a window takes it, and the writer.
pub struct SessionStore {
    saved: Option<Snapshot>,
    jobs: Option<Sender<Job>>,
}

impl Global for SessionStore {}

/// Reads the last session and starts the writer thread on `session.toml` (nothing is kept when
/// there is no data dir), and waits for the last write when the app quits.
pub fn init(cx: &mut App) {
    let path = Locations::resolve().map(|locations| locations.session());
    let saved = path.as_deref().and_then(file::read);
    let jobs = path.map(writer::spawn);
    cx.set_global(SessionStore { saved, jobs });
    cx.on_app_quit(|cx| {
        let jobs = cx.try_global::<SessionStore>().and_then(|store| store.jobs.clone());
        // Polled once every quit handler has run, so the window's last save is queued first.
        async move {
            let Some(jobs) = jobs else { return };
            let (reply, done) = oneshot::channel();
            if jobs.send(Job::Flush(reply)).is_ok() {
                let _ = done.await;
            }
        }
    })
    .detach();
}

/// Installs a store that hands out `saved` and writes nothing, for tests.
#[cfg(test)]
pub fn init_in_memory(saved: Option<Snapshot>, cx: &mut App) {
    cx.set_global(SessionStore { saved, jobs: None });
}

impl SessionStore {
    /// The service, if [`init`] ran (tests may not start it).
    pub fn get(cx: &App) -> Option<&Self> {
        cx.try_global::<Self>()
    }

    /// What the last run left, once: the first window takes it.
    pub fn take_saved(cx: &mut App) -> Option<Snapshot> {
        if !cx.has_global::<Self>() {
            return None;
        }
        cx.update_global::<Self, _>(|store, _| store.saved.take())
    }

    /// Writes `snapshot` in the background, in place of what was saved.
    pub fn save(&self, snapshot: Snapshot) {
        let Some(jobs) = &self.jobs else { return };
        if jobs.send(Job::Write(Box::new(snapshot))).is_err() {
            tracing::warn!("session: the writer thread is gone");
        }
    }

    /// Forgets the saved session (restoring or history was turned off).
    pub fn forget(&self) {
        self.save(Snapshot::default());
    }
}
