//! The history thread: it owns the database, so the UI thread never waits on disk (S1.7, edge
//! case "slow or network home directory"). Jobs run in the order they were sent, which is what
//! lets a session's saves name it before its row is known: `Begin` resolves the row first.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use futures::channel::oneshot;

use super::SessionId;
use super::resolve::{self, ORPHAN_LIFETIME, ResolveError};
use crate::app::media::Media;
use crate::db::media::{self, MediaId, Progress, Saved};
use crate::db::{Database, now_ms};
use crate::logging;

/// Work for the history thread.
pub(super) enum Job {
    /// Find or create `media`'s row for `session`, and reply with what it remembers.
    Begin { session: SessionId, media: Media, reply: oneshot::Sender<Option<Saved>> },
    /// `session`'s media started playing.
    Played(SessionId),
    /// Save `session`'s progress.
    Save(SessionId, Progress),
    /// `session` is over.
    End(SessionId),
    /// Mark `media` watched or unwatched.
    SetWatched(Media, bool),
    /// Forget everything.
    Clear,
    /// Work on the database for a table history doesn't own (saved playlists).
    Database(super::DatabaseWork),
    /// Reply once every job sent before this one is done.
    Flush(oneshot::Sender<()>),
}

/// Starts the thread, with the database at `path` (or in memory). It ends when every sender is
/// dropped.
pub(super) fn spawn(path: Option<PathBuf>) -> Sender<Job> {
    let (sender, receiver) = mpsc::channel();
    let started = std::thread::Builder::new().name("history".into()).spawn(move || {
        let database = path.map_or_else(Database::in_memory, |path| Database::open(&path));
        Worker::new(database).run(&receiver);
    });
    if let Err(error) = started {
        tracing::error!("history: couldn't start its thread: {error}");
    }
    sender
}

/// The thread's state: the database and the row of each running session.
pub(super) struct Worker {
    database: Database,
    sessions: HashMap<SessionId, MediaId>,
}

impl Worker {
    /// A worker on `database`, which first purges history orphaned long enough ago.
    pub(super) fn new(mut database: Database) -> Self {
        let before =
            now_ms().saturating_sub(i64::try_from(ORPHAN_LIFETIME.as_millis()).unwrap_or(0));
        match media::purge_orphans(database.connection(), before) {
            Ok(0) => {}
            Ok(purged) => tracing::info!("history: purged {purged} orphaned media"),
            Err(error) => tracing::warn!("history: couldn't purge orphaned media: {error}"),
        }
        Self { database, sessions: HashMap::new() }
    }

    fn run(mut self, receiver: &Receiver<Job>) {
        while let Ok(job) = receiver.recv() {
            self.handle(job);
        }
    }

    /// Does one job. Failures are logged: history is worth less than playing.
    pub(super) fn handle(&mut self, job: Job) {
        let result = match job {
            Job::Begin { session, media, reply } => {
                let saved = self.begin(session, &media);
                // The window may have closed meanwhile.
                let _ = reply.send(saved);
                Ok(())
            }
            Job::Played(session) => {
                self.with_row(session, |c, id| media::record_play(c, id, now_ms()))
            }
            Job::Save(session, progress) => {
                self.with_row(session, |c, id| media::save_progress(c, id, progress, now_ms()))
            }
            Job::End(session) => {
                self.sessions.remove(&session);
                Ok(())
            }
            Job::SetWatched(media, watched) => self.set_watched(&media, watched),
            Job::Clear => media::clear(self.database.connection()).map(drop),
            Job::Database(work) => {
                work(self.database.connection());
                Ok(())
            }
            Job::Flush(reply) => {
                let _ = reply.send(());
                Ok(())
            }
        };
        if let Err(error) = result {
            tracing::warn!("history: {error}");
        }
    }

    fn begin(&mut self, session: SessionId, media: &Media) -> Option<Saved> {
        let id = self.resolve(media)?;
        self.sessions.insert(session, id);
        media::saved(self.database.connection(), id)
            .inspect_err(|error| tracing::warn!("history: {error}"))
            .ok()
    }

    fn set_watched(&mut self, media: &Media, watched: bool) -> rusqlite::Result<()> {
        let Some(id) = self.resolve(media) else { return Ok(()) };
        media::set_watched(self.database.connection(), id, watched)
    }

    fn resolve(&mut self, media: &Media) -> Option<MediaId> {
        match resolve::resolve(self.database.connection(), media, now_ms()) {
            Ok(id) => Some(id),
            Err(ResolveError::Io(error)) => {
                if let Some(path) = media.as_path() {
                    tracing::debug!("history: can't identify {}: {error}", logging::path(path));
                }
                None
            }
            Err(ResolveError::Db(error)) => {
                tracing::warn!("history: {error}");
                None
            }
        }
    }

    /// Runs `write` on `session`'s row; nothing when the session has none (its media couldn't
    /// be identified).
    fn with_row(
        &mut self,
        session: SessionId,
        write: impl FnOnce(&rusqlite::Connection, MediaId) -> rusqlite::Result<()>,
    ) -> rusqlite::Result<()> {
        match self.sessions.get(&session) {
            Some(&id) => write(self.database.connection(), id),
            None => Ok(()),
        }
    }
}
