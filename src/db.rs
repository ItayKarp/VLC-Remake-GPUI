//! The local SQLite database (S1.7.1): `library.db` in the data dir, holding the playback history
//! ([`media`]), saved playlists ([`playlists`]) and the library (S4.1): its media folders and
//! their files ([`roots`], [`folders`], [`files`]), what is known about each media
//! ([`metadata`]), its cover ([`artwork`]) and thumbnails ([`thumbnails`]), and plays, visits
//! and favorites ([`activity`]) with the media Home lists ([`played`]), and the search index
//! ([`search`]). Network streams are in [`network`], each subtitle track's remembered sync in
//! [`subtitle_sync`], and the subtitle files linked to each media in [`subtitle_links`].
//!
//! Opening never fails. The file is checked with `PRAGMA integrity_check` and brought up to date
//! by [`migrations`]; a damaged one is moved aside to `library.db.corrupt` and started over, since
//! losing history is acceptable and crashing is not. When even that fails (no permission, or a
//! database from a newer version of the app, which must not be thrown away), history lives in
//! memory until the app quits. Writes go through WAL, so a crash loses at most the transaction
//! in flight, and another process reading or writing waits up to [`BUSY_TIMEOUT`].
//!
//! Every call blocks on disk: use the database from a background thread only.

pub mod activity;
pub mod artwork;
pub mod files;
pub mod folders;
pub mod media;
pub mod metadata;
mod migrations;
pub mod network;
pub mod played;
pub mod playlists;
pub mod roots;
pub mod search;
pub mod songs;
pub mod subtitle_links;
pub mod subtitle_sync;
#[cfg(test)]
mod tests;
mod text_enum;
pub mod thumbnails;

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, ErrorCode};

use crate::logging;

/// How long a statement waits for another connection's lock before failing.
pub const BUSY_TIMEOUT: Duration = Duration::from_secs(2);
/// Appended to a damaged database's name when it is moved aside.
const CORRUPT_SUFFIX: &str = ".corrupt";
/// The files SQLite keeps next to a WAL database, which belong to it.
const SIDE_FILE_SUFFIXES: [&str; 2] = ["-wal", "-shm"];

/// An open, migrated database.
pub struct Database {
    connection: Connection,
}

/// Why a database file couldn't be used.
#[derive(Debug)]
enum Unusable {
    /// The file is damaged or isn't a database: move it aside and start over.
    Corrupt(String),
    /// The file may be fine but can't be used now: leave it alone.
    Unavailable(String),
}

impl std::fmt::Display for Unusable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Corrupt(why) | Self::Unavailable(why) => f.write_str(why),
        }
    }
}

impl Database {
    /// Opens the database at `path`, creating and migrating it as needed. Never fails: see the
    /// module comment for what happens to a damaged or unusable file.
    pub fn open(path: &Path) -> Self {
        let connection = match open_file(path) {
            Ok(connection) => connection,
            Err(Unusable::Corrupt(why)) => {
                tracing::warn!("db: {} is damaged ({why}); starting over", logging::path(path));
                move_aside(path);
                open_file(path).unwrap_or_else(|error| in_memory_after(&error))
            }
            Err(error) => in_memory_after(&error),
        };
        Self { connection }
    }

    /// A database that lives in memory only, for tests and when no file can be used.
    pub fn in_memory() -> Self {
        let connection = open_in_memory().expect("an in-memory SQLite database always opens");
        Self { connection }
    }

    /// The connection, for the table modules.
    pub fn connection(&mut self) -> &mut Connection {
        &mut self.connection
    }
}

/// Opens, checks and migrates the file at `path`.
fn open_file(path: &Path) -> Result<Connection, Unusable> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|error| Unusable::Unavailable(error.to_string()))?;
    }
    let mut connection = Connection::open(path).map_err(|error| classify(&error))?;
    connection.busy_timeout(BUSY_TIMEOUT).map_err(|error| classify(&error))?;
    connection.pragma_update(None, "journal_mode", "WAL").map_err(|error| classify(&error))?;
    // With WAL, NORMAL survives an app crash; only a power cut can lose the last commit.
    connection.pragma_update(None, "synchronous", "NORMAL").map_err(|error| classify(&error))?;
    enforce_foreign_keys(&connection)?;
    check_integrity(&connection)?;
    migrations::run(&mut connection)?;
    Ok(connection)
}

fn open_in_memory() -> Result<Connection, Unusable> {
    let mut connection = Connection::open_in_memory().map_err(|error| classify(&error))?;
    enforce_foreign_keys(&connection)?;
    migrations::run(&mut connection)?;
    Ok(connection)
}

/// Turns on what the library's `REFERENCES` clauses promise: deleting a folder takes its files
/// with it, and deleting a media row its plays. SQLite leaves this off unless asked, per
/// connection.
fn enforce_foreign_keys(connection: &Connection) -> Result<(), Unusable> {
    connection.pragma_update(None, "foreign_keys", true).map_err(|error| classify(&error))
}

/// `PRAGMA integrity_check` says `ok`, or lists what is wrong.
fn check_integrity(connection: &Connection) -> Result<(), Unusable> {
    let verdict: String = connection
        .query_row("PRAGMA integrity_check(1)", [], |row| row.get(0))
        .map_err(|error| classify(&error))?;
    if verdict == "ok" { Ok(()) } else { Err(Unusable::Corrupt(verdict)) }
}

/// Whether a SQLite error means the file is damaged, or only unusable right now.
fn classify(error: &rusqlite::Error) -> Unusable {
    match error.sqlite_error_code() {
        Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase) => {
            Unusable::Corrupt(error.to_string())
        }
        _ => Unusable::Unavailable(error.to_string()),
    }
}

/// Renames the damaged database and its side files to `….corrupt`, replacing an older backup,
/// so it can still be looked at.
fn move_aside(path: &Path) {
    let suffixes = std::iter::once("").chain(SIDE_FILE_SUFFIXES);
    for suffix in suffixes {
        let from = with_suffix(path, suffix);
        if !from.exists() {
            continue;
        }
        let to = with_suffix(path, &format!("{CORRUPT_SUFFIX}{suffix}"));
        if let Err(error) = std::fs::rename(&from, &to) {
            tracing::warn!("db: couldn't move {} aside: {error}", logging::path(&from));
            // A side file left behind would be replayed into the new database.
            let _ = std::fs::remove_file(&from);
        }
    }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Whether `error` comes from the file's surroundings (a lock, permissions, a full disk) rather
/// than from its contents.
fn is_transient(error: &rusqlite::Error) -> bool {
    matches!(
        error.sqlite_error_code(),
        Some(
            ErrorCode::DatabaseBusy
                | ErrorCode::DatabaseLocked
                | ErrorCode::CannotOpen
                | ErrorCode::ReadOnly
                | ErrorCode::DiskFull
                | ErrorCode::SystemIoFailure
                | ErrorCode::PermissionDenied
                | ErrorCode::OutOfMemory
        )
    )
}

fn in_memory_after(error: &Unusable) -> Connection {
    tracing::error!("db: can't use the database file ({error}); history won't be kept");
    open_in_memory().expect("an in-memory SQLite database always opens")
}

/// Now, in milliseconds since the Unix epoch, as the tables store times.
pub fn now_ms() -> i64 {
    let since = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    i64::try_from(since.as_millis()).unwrap_or(i64::MAX)
}
