//! How the indexer reaches the database: the history thread owns the connection, so the indexer
//! sends it work and waits for the answer (it runs on a thread of its own, so waiting is fine).
//! Tests use an in-memory database run in place.

use std::sync::mpsc;

use rusqlite::Connection;

use crate::history::{DatabaseHandle, DatabaseWork};

/// Runs work on the library's database.
pub trait Store: Send {
    /// Runs `work`, now or later; `false` when the database is gone (the app is quitting).
    fn run(&self, work: DatabaseWork) -> bool;
}

impl Store for DatabaseHandle {
    fn run(&self, work: DatabaseWork) -> bool {
        DatabaseHandle::run(self, work)
    }
}

/// Runs `work` on `store` and waits for its result. `None` when it failed (logged) or the
/// database is gone.
pub fn call<R: Send + 'static>(
    store: &dyn Store,
    work: impl FnOnce(&mut Connection) -> rusqlite::Result<R> + Send + 'static,
) -> Option<R> {
    let (reply, answer) = mpsc::channel();
    let sent = store.run(Box::new(move |connection| {
        let _ = reply.send(work(connection));
    }));
    if !sent {
        return None;
    }
    match answer.recv() {
        Ok(Ok(value)) => Some(value),
        Ok(Err(error)) => {
            tracing::warn!("library: {error}");
            None
        }
        Err(_) => None,
    }
}

/// An in-memory database that runs work in place, for tests.
#[cfg(test)]
pub struct InPlace(pub std::sync::Mutex<crate::db::Database>);

#[cfg(test)]
impl InPlace {
    /// A fresh in-memory library.
    pub fn new() -> Self {
        Self(std::sync::Mutex::new(crate::db::Database::in_memory()))
    }
}

#[cfg(test)]
impl Store for InPlace {
    fn run(&self, work: DatabaseWork) -> bool {
        work(self.0.lock().expect("not poisoned").connection());
        true
    }
}
