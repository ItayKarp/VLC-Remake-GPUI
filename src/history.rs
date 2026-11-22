//! Playback history (S1.7): what was played, where it was left, and whether it was watched,
//! per media identity, in the [`db`](crate::db) on the history thread ([`worker`]).
//!
//! - [`History`]: the GPUI global; [`init`] starts it.
//! - [`Session`]: one playback of one media, started by [`History::begin`]. It saves its
//!   progress when told to and when dropped, so closing the window or quitting keeps the
//!   position.
//! - [`identity`] and [`resolve`]: how a file is recognized after a move (S1.7.2).
//! - [`policy`]: what is worth keeping and where to resume (S1.7.4).
//!
//! The history thread owns the database, so other tables' modules (saved playlists) send their
//! work through [`History::with_database`] rather than opening a second connection that would
//! race this one's migrations.
//!
//! With "Save recently played items" off, callers don't begin sessions, so nothing is written.

pub mod identity;
pub mod policy;
pub mod resolve;
#[cfg(test)]
mod tests;
mod worker;

use std::cell::Cell;
use std::sync::mpsc::Sender;

use futures::channel::oneshot;
use gpui::{App, Global};

use self::worker::Job;
use crate::app::media::Media;
use crate::db::media::{Progress, Saved};
use crate::settings::Locations;

/// Work for the history thread's database connection; it reports its own outcome.
pub(crate) type DatabaseWork = Box<dyn FnOnce(&mut rusqlite::Connection) + Send>;

/// Names one [`Session`] to the history thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SessionId(u64);

/// The history service: a channel to the thread that owns the database.
pub struct History {
    jobs: Sender<Job>,
    next_session: Cell<u64>,
}

impl Global for History {}

/// Starts the history thread on `library.db` in the data dir (in memory when there is none),
/// and waits for its writes when the app quits.
pub fn init(cx: &mut App) {
    let path = Locations::resolve().map(|locations| locations.database());
    cx.set_global(History::new(worker::spawn(path)));
    cx.on_app_quit(|cx| {
        let jobs = cx.try_global::<History>().map(|history| history.jobs.clone());
        // An async block runs nothing until polled, which GPUI does once every quit handler has
        // run and the windows are gone: the sessions they ended are queued before the flush.
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

impl History {
    fn new(jobs: Sender<Job>) -> Self {
        Self { jobs, next_session: Cell::new(0) }
    }

    /// The service, if [`init`] ran (tests may not start it).
    pub fn get(cx: &App) -> Option<&Self> {
        cx.try_global::<Self>()
    }

    /// Starts a playback of `media`. The receiver gets what was saved about it, or `None` when it
    /// can't be identified (a missing file) or history failed.
    pub fn begin(&self, media: Media) -> (Session, oneshot::Receiver<Option<Saved>>) {
        let id = SessionId(self.next_session.get());
        self.next_session.set(id.0 + 1);
        let (reply, saved) = oneshot::channel();
        self.send(Job::Begin { session: id, media, reply });
        (Session { id, jobs: self.jobs.clone(), unsaved: None }, saved)
    }

    /// Marks `media` watched or unwatched (S1.7.5); either way it plays from the start next time.
    pub fn set_watched(&self, media: Media, watched: bool) {
        self.send(Job::SetWatched(media, watched));
    }

    /// Forgets every media's history (S1.7.5). Running sessions stop recording.
    pub fn clear(&self) {
        self.send(Job::Clear);
    }

    /// Forgets every saved position, keeping plays and watched marks (S5.5).
    pub fn forget_positions(&self) {
        self.send(Job::ForgetPositions);
    }

    /// Runs `query` on the database and hands back its answer; `None` when it failed (it is
    /// logged) or the thread is gone.
    pub(crate) fn query<T: Send + 'static>(
        &self,
        name: &'static str,
        query: impl FnOnce(&rusqlite::Connection) -> rusqlite::Result<T> + Send + 'static,
    ) -> oneshot::Receiver<Option<T>> {
        let (reply, answer) = oneshot::channel();
        self.with_database(move |connection| {
            let result = query(connection).inspect_err(|error| tracing::warn!("{name}: {error}"));
            // The asker may be gone.
            let _ = reply.send(result.ok());
        });
        answer
    }

    /// Runs `work` on the database, after the jobs sent before it.
    pub(crate) fn with_database(
        &self,
        work: impl FnOnce(&mut rusqlite::Connection) + Send + 'static,
    ) {
        self.send(Job::Database(Box::new(work)));
    }

    /// A handle that runs work on the database from any thread, for the library's indexer.
    pub(crate) fn database(&self) -> DatabaseHandle {
        DatabaseHandle(self.jobs.clone())
    }

    fn send(&self, job: Job) {
        if self.jobs.send(job).is_err() {
            tracing::warn!("history: the history thread is gone");
        }
    }
}

/// Runs work on the history thread's database from another thread (the library's indexer),
/// after the jobs sent before it.
#[derive(Clone)]
pub(crate) struct DatabaseHandle(Sender<Job>);

impl DatabaseHandle {
    /// Sends `work`; `false` when the history thread is gone.
    pub(crate) fn run(&self, work: DatabaseWork) -> bool {
        self.0.send(Job::Database(work)).is_ok()
    }
}

/// One playback of one media. Dropping it saves the last progress it was given.
pub struct Session {
    id: SessionId,
    jobs: Sender<Job>,
    /// Progress given since the last save.
    unsaved: Option<Progress>,
}

impl Session {
    /// The id, to tell a reply to this session from one to an earlier session.
    pub(crate) fn id(&self) -> SessionId {
        self.id
    }

    /// The media started playing: counts a play.
    pub fn played(&self) {
        let _ = self.jobs.send(Job::Played(self.id));
    }

    /// Remembers `progress`, to save with the next [`Self::save`] or on drop.
    pub fn update(&mut self, progress: Progress) {
        self.unsaved = Some(progress);
    }

    /// Saves the progress given since the last save, if any.
    pub fn save(&mut self) {
        if let Some(progress) = self.unsaved.take() {
            let _ = self.jobs.send(Job::Save(self.id, progress));
        }
    }

    /// Ends the session without saving what wasn't saved yet (history was just turned off).
    pub fn discard(mut self) {
        self.unsaved = None;
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.save();
        // The thread may already be gone when the app quits.
        let _ = self.jobs.send(Job::End(self.id));
    }
}
