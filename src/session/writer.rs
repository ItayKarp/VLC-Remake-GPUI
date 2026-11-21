//! The session's writer thread: turns snapshots into `session.toml` off the UI thread (a long
//! queue is a lot of text, and the data dir may be slow). Writes are coalesced: when several
//! snapshots are waiting, only the newest is written. Each write replaces the file atomically.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use futures::channel::oneshot;

use super::{Snapshot, file};
use crate::settings;

/// Work for the writer thread.
pub(super) enum Job {
    /// Save this snapshot.
    Write(Box<Snapshot>),
    /// Reply once every snapshot sent before this one is written.
    Flush(oneshot::Sender<()>),
}

/// Starts the thread, writing to `path`. It ends when every sender is dropped.
pub(super) fn spawn(path: PathBuf) -> Sender<Job> {
    let (sender, receiver) = mpsc::channel();
    let started =
        std::thread::Builder::new().name("session".into()).spawn(move || run(&path, &receiver));
    if let Err(error) = started {
        tracing::error!("session: couldn't start its thread: {error}");
    }
    sender
}

fn run(path: &Path, jobs: &Receiver<Job>) {
    while let Ok(first) = jobs.recv() {
        let mut newest = None;
        let mut flushes = Vec::new();
        for job in std::iter::once(first).chain(jobs.try_iter()) {
            match job {
                Job::Write(snapshot) => newest = Some(snapshot),
                Job::Flush(reply) => flushes.push(reply),
            }
        }
        if let Some(snapshot) = newest {
            write(path, &snapshot);
        }
        for reply in flushes {
            let _ = reply.send(());
        }
    }
}

fn write(path: &Path, snapshot: &Snapshot) {
    let written = file::to_text(snapshot)
        .map_err(|error| error.to_string())
        .and_then(|text| settings::write_atomically(path, &text).map_err(|e| e.to_string()));
    if let Err(error) = written {
        tracing::warn!("session: couldn't save the session: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::media::Media;

    #[test]
    fn the_newest_snapshot_is_on_disk_after_a_flush() {
        let dir = std::env::temp_dir().join(format!("rvlc-session-{}", std::process::id()));
        let path = dir.join("session.toml");
        let jobs = spawn(path.clone());
        for name in ["a", "b", "c"] {
            let media = vec![Media::Url(format!("https://x/{name}"))];
            jobs.send(Job::Write(Box::new(Snapshot { media, ..Snapshot::default() }))).unwrap();
        }
        let (reply, done) = oneshot::channel();
        jobs.send(Job::Flush(reply)).unwrap();
        futures::executor::block_on(done).unwrap();
        let saved = file::read(&path).expect("a saved session");
        assert_eq!(saved.media, [Media::Url("https://x/c".into())]);
        std::fs::remove_dir_all(dir).ok();
    }
}
