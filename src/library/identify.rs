//! Identifying the files a scan found (S4.2.2, S4.2.3): each one's content key (S1.7.2) finds
//! the media row it is, so a file moved or renamed inside the library keeps its history, and
//! copies are one media.
//!
//! - Only files a scan added or saw change are identified; an unchanged file is never read again.
//! - A file still being written (a download, a recording) is left until its size and modification
//!   time have held still for [`STABLE_AFTER`]; the indexer comes back to it.
//! - Online-only files are never read (see [`placeholder`](super::placeholder)).
//! - A media row no file points at any more (its file moved) follows the file to its new place,
//!   which is where history looks first.

use std::fs;
use std::path::{Path, PathBuf};
use std::slice;
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use rusqlite::Connection;

use super::walk::MAX_WORKERS;
use crate::db::files::{self, FileId, Unidentified};
use crate::db::media::{self, Entry, FileStamp, MediaId};
use crate::db::metadata;
use crate::history::identity;
use crate::opening::Cancel;
use crate::settings::stored_path;

/// How long a file's size and modification time must hold still before it is read.
pub const STABLE_AFTER: Duration = Duration::from_secs(3);

/// A file and the content key it was read to have.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identified {
    /// The file's row.
    pub id: FileId,
    /// Where it is.
    pub path: PathBuf,
    /// Its size and modification time when it was read.
    pub stamp: FileStamp,
    /// Its content key.
    pub key: String,
}

/// What reading one batch of files gave.
#[derive(Debug, Default)]
pub struct Read {
    /// The files that were read.
    pub identified: Vec<Identified>,
    /// Whether some files were still changing, so a later pass should try them again.
    pub unsettled: bool,
}

/// Reads the content keys of `batch` on a few threads, at `now_ms`. Files that can't be read
/// (gone, no permission) are left out; ones still changing are left for later.
pub fn read(batch: &[Unidentified], now_ms: i64, cancel: &Cancel) -> Read {
    let next = Mutex::new(batch.iter());
    let outcome = Mutex::new(Read::default());
    thread::scope(|scope| {
        for _ in 0..MAX_WORKERS.min(batch.len()) {
            scope.spawn(|| read_shared(&next, &outcome, now_ms, cancel));
        }
    });
    outcome.into_inner().unwrap_or_else(PoisonError::into_inner)
}

/// One reading thread: takes files from `next` until none are left, adding to `outcome`.
fn read_shared(
    next: &Mutex<slice::Iter<'_, Unidentified>>,
    outcome: &Mutex<Read>,
    now_ms: i64,
    cancel: &Cancel,
) {
    while let Some(file) = next.lock().unwrap_or_else(PoisonError::into_inner).next() {
        if cancel.is_cancelled() {
            return;
        }
        let result = read_one(file, now_ms);
        let mut outcome = outcome.lock().unwrap_or_else(PoisonError::into_inner);
        match result {
            Ok(identified) => outcome.identified.push(identified),
            Err(Skipped::Unsettled) => outcome.unsettled = true,
            Err(Skipped::Unreadable) => {}
        }
    }
}

/// Why a file wasn't read.
enum Skipped {
    /// It is still changing, or changed since the scan.
    Unsettled,
    /// It is gone or can't be read; the next scan sorts it out.
    Unreadable,
}

fn read_one(file: &Unidentified, now_ms: i64) -> Result<Identified, Skipped> {
    let metadata = fs::metadata(&file.path).map_err(|_| Skipped::Unreadable)?;
    let stamp = identity::stamp(&metadata);
    if stamp != file.stamp || !has_settled(stamp, now_ms) {
        return Err(Skipped::Unsettled);
    }
    let key = identity::file_key(&file.path, stamp.size).map_err(|error| {
        tracing::debug!("library: can't read {}: {error}", crate::logging::path(&file.path));
        Skipped::Unreadable
    })?;
    Ok(Identified { id: file.id, path: file.path.clone(), stamp, key })
}

/// Whether a file last modified at `stamp` has held still long enough by `now_ms`.
fn has_settled(stamp: FileStamp, now_ms: i64) -> bool {
    let stable_ms = STABLE_AFTER.as_millis() as i64;
    stamp.modified_ms.is_none_or(|modified| now_ms.saturating_sub(modified) >= stable_ms)
}

/// Points each identified file at its media row, creating rows for new media, in one
/// transaction.
pub fn link(connection: &mut Connection, identified: &[Identified]) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    for file in identified {
        let media = media_for(&transaction, &file.path, file.stamp, &file.key)?;
        files::set_media(&transaction, file.id, media, file.stamp)?;
        // A file identified again (Resync, a move) has its tags read again too.
        metadata::reprobe(&transaction, media)?;
    }
    transaction.commit()
}

/// The media row of the file at `path` with content `key`: the one with that key (moved here
/// when no file points at it any more), or a new one.
fn media_for(
    connection: &Connection,
    path: &Path,
    stamp: FileStamp,
    key: &str,
) -> rusqlite::Result<MediaId> {
    let entry =
        Entry { identity: key.to_owned(), location: stored_path::encode(path), stamp: Some(stamp) };
    let Some(id) = media::find_identity(connection, key)? else {
        return media::insert(connection, &entry);
    };
    if files::of_media(connection, id)?.is_empty() {
        media::relocate(connection, id, &entry)?;
    }
    Ok(id)
}

#[cfg(test)]
mod tests;
