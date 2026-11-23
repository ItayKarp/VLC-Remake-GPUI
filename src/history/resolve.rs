//! Finding a media's row in the `media` table (S1.7.2), creating it if it's new.
//!
//! - A file whose location and stamp (size, modification time) match a row is that row, without
//!   reading the file.
//! - Otherwise its content key decides: a row with that key, wherever it was, is moved here (a
//!   renamed or moved file, or another copy of it).
//! - A file changed in place (its key no longer matches the row at its location) is new media,
//!   and the old row is orphaned; it is purged after [`ORPHAN_LIFETIME`] unless the file comes
//!   back. A file only touched (same key, new modification time) stays the same media.
//! - A URL is its own identity.
//!
//! [`identity_of`] only reads: it names a media's identity without making or moving a row.

use std::io;
use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;

use super::identity;
use crate::app::media::Media;
use crate::db::media::{self, Entry, MediaId};
use crate::settings::stored_path;

/// How long the history of a file changed in place is kept.
pub const ORPHAN_LIFETIME: Duration = Duration::from_hours(30 * 24);

/// Why a media has no row.
#[derive(Debug)]
pub enum ResolveError {
    /// The file can't be read.
    Io(io::Error),
    /// The database failed.
    Db(rusqlite::Error),
}

impl From<io::Error> for ResolveError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<rusqlite::Error> for ResolveError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Db(error)
    }
}

/// The row for `media`, created if needed; `now_ms` dates an orphaned row.
pub fn resolve(
    connection: &mut Connection,
    media: &Media,
    now_ms: i64,
) -> Result<MediaId, ResolveError> {
    match media {
        Media::File(path) => resolve_file(connection, path, now_ms),
        Media::Url(url) => {
            let entry =
                Entry { identity: identity::url_key(url), location: url.clone(), stamp: None };
            let transaction = connection.transaction()?;
            let id = match media::find_identity(&transaction, &entry.identity)? {
                Some(id) => id,
                None => media::insert(&transaction, &entry)?,
            };
            transaction.commit()?;
            Ok(id)
        }
    }
}

/// The identity of `media`: the one its row holds when the file is unchanged since it was last
/// matched, else its content key, read from the file. Writes nothing.
pub fn identity_of(connection: &Connection, media: &Media) -> Result<String, ResolveError> {
    let path = match media {
        Media::File(path) => path,
        Media::Url(url) => return Ok(identity::url_key(url)),
    };
    let stamp = identity::stamp(&std::fs::metadata(path)?);
    let here = media::find_at(connection, &stored_path::encode(path))?;
    match here.filter(|row| row.stamp == Some(stamp)) {
        Some(row) => Ok(row.identity),
        None => Ok(identity::file_key(path, stamp.size)?),
    }
}

fn resolve_file(
    connection: &mut Connection,
    path: &Path,
    now_ms: i64,
) -> Result<MediaId, ResolveError> {
    let location = stored_path::encode(path);
    let stamp = identity::stamp(&std::fs::metadata(path)?);
    let here = media::find_at(connection, &location)?;
    if let Some(row) = &here
        && row.stamp == Some(stamp)
    {
        return Ok(row.id);
    }
    // Hashed outside the transaction: it reads the file, which may be slow.
    let identity = identity::file_key(path, stamp.size)?;
    let transaction = connection.transaction()?;
    if let Some(row) = here.filter(|row| row.identity != identity) {
        media::orphan(&transaction, row.id, now_ms)?;
    }
    let entry = Entry { identity, location, stamp: Some(stamp) };
    let id = match media::find_identity(&transaction, &entry.identity)? {
        Some(id) => {
            media::relocate(&transaction, id, &entry)?;
            id
        }
        None => media::insert(&transaction, &entry)?,
    };
    transaction.commit()?;
    Ok(id)
}
