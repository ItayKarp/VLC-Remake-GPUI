//! A [`History`] for other modules' tests. Its jobs run on the test's own thread, on an
//! in-memory database, when the test settles the app, so work that goes through the history
//! thread is tested without a thread, a file or a wait.

use std::sync::mpsc::{self, Receiver};

use gpui::TestAppContext;

use super::super::History;
use super::super::worker::{Job, Worker};
use crate::db::Database;

/// The history thread's stand-in: the jobs sent to the [`History`] global, and the worker that
/// runs them.
pub(crate) struct TestHistory {
    jobs: Receiver<Job>,
    worker: Worker,
}

impl TestHistory {
    /// Makes a [`History`] the app's global, on a new in-memory database.
    pub(crate) fn install(cx: &mut TestAppContext) -> Self {
        let (sender, jobs) = mpsc::channel();
        cx.update(|cx| cx.set_global(History::new(sender)));
        Self { jobs, worker: Worker::new(Database::in_memory()) }
    }

    /// Runs the app's tasks and the database's jobs, in turn, until neither has anything left.
    pub(crate) fn settle(&mut self, cx: &mut TestAppContext) {
        loop {
            cx.run_until_parked();
            let mut ran = false;
            while let Ok(job) = self.jobs.try_recv() {
                self.worker.handle(job);
                ran = true;
            }
            if !ran {
                return;
            }
        }
    }

    /// Runs `query` on the database now, for a test to look at what was written.
    pub(crate) fn read<T: Send + 'static>(
        &mut self,
        query: impl FnOnce(&rusqlite::Connection) -> T + Send + 'static,
    ) -> T {
        let (reply, answer) = mpsc::channel();
        self.worker.handle(Job::Database(Box::new(move |connection| {
            let _ = reply.send(query(connection));
        })));
        answer.recv().expect("the job ran")
    }
}
