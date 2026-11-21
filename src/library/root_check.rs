//! Whether a media folder can be indexed now (S4.2.6): there, readable, and answering in time.
//!
//! The check reads the folder on a thread of its own and waits at most [`CHECK_TIMEOUT`], so a
//! network share that hangs can't stall the indexer: it is marked Slow and tried again at the
//! next reconcile. One that answers, but only after [`SLOW_AFTER`], is Slow and still indexed.

use std::fs;
use std::io;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::db::roots::RootStatus;

/// How long a folder may take to answer before the check gives up on it for now.
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(10);
/// A folder that takes longer than this to answer is on a slow (network) drive.
pub const SLOW_AFTER: Duration = Duration::from_secs(2);

/// What a check found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Checked {
    /// The folder's status.
    pub status: RootStatus,
    /// Whether it can be scanned now (a folder that didn't answer in time can't).
    pub scannable: bool,
}

/// Checks the folder at `path`.
pub fn check(path: &Path) -> Checked {
    let (reply, answer) = mpsc::channel();
    let target = path.to_path_buf();
    let started = Instant::now();
    let spawned = std::thread::Builder::new().name("library-check".into()).spawn(move || {
        let _ = reply.send(read_first_entry(&target));
    });
    if spawned.is_err() {
        return classify(read_first_entry(path), started.elapsed());
    }
    match answer.recv_timeout(CHECK_TIMEOUT) {
        Ok(result) => classify(result, started.elapsed()),
        Err(_) => Checked { status: RootStatus::Slow, scannable: false },
    }
}

fn read_first_entry(path: &Path) -> io::Result<()> {
    let mut entries = fs::read_dir(path)?;
    entries.next().transpose().map(drop)
}

/// The status of a folder whose read gave `result` after `elapsed`.
pub fn classify(result: io::Result<()>, elapsed: Duration) -> Checked {
    let status = match result {
        Ok(()) if elapsed >= SLOW_AFTER => RootStatus::Slow,
        Ok(()) => RootStatus::Ok,
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            RootStatus::PermissionDenied
        }
        // Gone, not a folder, or a drive that failed: there's nothing to index.
        Err(_) => RootStatus::Missing,
    };
    let scannable = matches!(status, RootStatus::Ok | RootStatus::Slow);
    Checked { status, scannable }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opening::scratch::scratch_dir;

    #[test]
    fn folders_are_checked_for_what_they_are() {
        let dir = scratch_dir();
        assert_eq!(check(&dir), Checked { status: RootStatus::Ok, scannable: true });
        let missing = check(&dir.join("gone"));
        assert_eq!(missing, Checked { status: RootStatus::Missing, scannable: false });
        fs::write(dir.join("file"), b"x").expect("write");
        assert_eq!(check(&dir.join("file")).status, RootStatus::Missing);
    }

    #[test]
    fn slow_and_refused_folders_are_told_apart() {
        let fast = Duration::from_millis(5);
        let denied = io::Error::from(io::ErrorKind::PermissionDenied);
        assert_eq!(classify(Err(denied), fast).status, RootStatus::PermissionDenied);
        let slow = classify(Ok(()), SLOW_AFTER);
        assert_eq!(slow, Checked { status: RootStatus::Slow, scannable: true });
    }
}
