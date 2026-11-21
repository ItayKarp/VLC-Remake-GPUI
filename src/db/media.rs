//! The `media` table (S1.7.1): one row per media identity, with where it was last seen and what
//! the player remembers about it. How a file finds its row is `history::resolve`'s job; this
//! module only reads and writes rows.
//!
//! Since S4.1 a row is also the library's media: [`metadata`](super::metadata) writes its tags,
//! and [`files`](super::files) point at it. Clearing the history forgets what was played, not
//! what the library holds.

use std::time::Duration;

use rusqlite::{Connection, OptionalExtension as _, Row, params};

/// A row of the `media` table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MediaId(pub i64);

/// A file's size and modification time, which say cheaply whether it changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileStamp {
    /// The size in bytes.
    pub size: u64,
    /// The modification time in milliseconds since the Unix epoch, if the file system has one.
    pub modified_ms: Option<i64>,
}

/// What identifies a row and where its media is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The content key of a file, or `url:` and the URL.
    pub identity: String,
    /// A stored path or a URL.
    pub location: String,
    /// The file's stamp; `None` for a URL.
    pub stamp: Option<FileStamp>,
}

/// The row living at a location.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Located {
    /// The row.
    pub id: MediaId,
    /// Its identity.
    pub identity: String,
    /// The stamp the file had when the row was last matched to it.
    pub stamp: Option<FileStamp>,
}

/// What the player remembers about a media.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Saved {
    /// Its length, when it was last played.
    pub duration: Option<Duration>,
    /// Where to resume; `None` starts from the beginning.
    pub position: Option<Duration>,
    /// Whether it was watched to the end (or marked so).
    pub watched: bool,
    /// How many times it started playing.
    pub play_count: u32,
}

/// What a save during playback writes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    /// Where to resume next time; `None` starts from the beginning.
    pub position: Option<Duration>,
    /// The media's length, if known.
    pub duration: Option<Duration>,
    /// Whether playback got far enough to count as watched. Never unmarks.
    pub watched: bool,
}

/// The live row at `location`, if any. Orphaned rows don't live anywhere.
pub fn find_at(connection: &Connection, location: &str) -> rusqlite::Result<Option<Located>> {
    connection
        .query_row(
            "SELECT id, identity, size, modified_ms FROM media
             WHERE location = ?1 AND orphaned_ms IS NULL
             ORDER BY last_played_ms DESC LIMIT 1",
            [location],
            |row| {
                let size: Option<i64> = row.get(2)?;
                let modified_ms: Option<i64> = row.get(3)?;
                let stamp = size.map(|size| FileStamp { size: size as u64, modified_ms });
                Ok(Located { id: MediaId(row.get(0)?), identity: row.get(1)?, stamp })
            },
        )
        .optional()
}

/// The row for `identity`: a live one first, then the most recently played.
pub fn find_identity(connection: &Connection, identity: &str) -> rusqlite::Result<Option<MediaId>> {
    connection
        .query_row(
            "SELECT id FROM media WHERE identity = ?1
             ORDER BY orphaned_ms IS NOT NULL, last_played_ms DESC LIMIT 1",
            [identity],
            |row| row.get(0).map(MediaId),
        )
        .optional()
}

/// Adds a row for `entry`.
pub fn insert(connection: &Connection, entry: &Entry) -> rusqlite::Result<MediaId> {
    let (size, modified_ms) = stamp_columns(entry.stamp);
    connection.execute(
        "INSERT INTO media (identity, location, size, modified_ms, added_ms)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![entry.identity, entry.location, size, modified_ms, super::now_ms()],
    )?;
    Ok(MediaId(connection.last_insert_rowid()))
}

/// Moves row `id` to `entry`'s location and stamp, bringing it back if it was orphaned.
pub fn relocate(connection: &Connection, id: MediaId, entry: &Entry) -> rusqlite::Result<()> {
    let (size, modified_ms) = stamp_columns(entry.stamp);
    connection.execute(
        "UPDATE media SET location = ?2, size = ?3, modified_ms = ?4, orphaned_ms = NULL
         WHERE id = ?1",
        params![id.0, entry.location, size, modified_ms],
    )?;
    Ok(())
}

/// Marks row `id` as orphaned at `now_ms`: its file became other media.
pub fn orphan(connection: &Connection, id: MediaId, now_ms: i64) -> rusqlite::Result<()> {
    connection.execute("UPDATE media SET orphaned_ms = ?2 WHERE id = ?1", params![id.0, now_ms])?;
    Ok(())
}

/// Deletes the rows orphaned before `before_ms`.
pub fn purge_orphans(connection: &Connection, before_ms: i64) -> rusqlite::Result<usize> {
    connection.execute("DELETE FROM media WHERE orphaned_ms < ?1", [before_ms])
}

/// What row `id` remembers.
pub fn saved(connection: &Connection, id: MediaId) -> rusqlite::Result<Saved> {
    connection.query_row(
        "SELECT duration_ms, position_ms, watched, play_count FROM media WHERE id = ?1",
        [id.0],
        read_saved,
    )
}

/// Counts a play of row `id`, at `now_ms`, and keeps it in `plays` for Home (S4.9).
pub fn record_play(connection: &Connection, id: MediaId, now_ms: i64) -> rusqlite::Result<()> {
    connection.execute(
        "UPDATE media SET play_count = play_count + 1, last_played_ms = ?2 WHERE id = ?1",
        params![id.0, now_ms],
    )?;
    connection.execute(
        "INSERT INTO plays (media_id, played_ms) VALUES (?1, ?2)",
        params![id.0, now_ms],
    )?;
    Ok(())
}

/// Writes `progress` to row `id`, at `now_ms`.
pub fn save_progress(
    connection: &Connection,
    id: MediaId,
    progress: Progress,
    now_ms: i64,
) -> rusqlite::Result<()> {
    connection.execute(
        "UPDATE media SET position_ms = ?2, duration_ms = COALESCE(?3, duration_ms),
             watched = MAX(watched, ?4), last_played_ms = ?5
         WHERE id = ?1",
        params![
            id.0,
            progress.position.map(millis),
            progress.duration.map(millis),
            progress.watched,
            now_ms
        ],
    )?;
    Ok(())
}

/// Marks row `id` watched or unwatched. Either way it starts from the beginning next time.
pub fn set_watched(connection: &Connection, id: MediaId, watched: bool) -> rusqlite::Result<()> {
    connection.execute(
        "UPDATE media SET watched = ?2, position_ms = NULL WHERE id = ?1",
        params![id.0, watched],
    )?;
    Ok(())
}

/// Forgets what was played: positions, watched marks, plays and visits. Media the library
/// holds (files point at it) or that is a favorite keeps its row, and with it its tags; the rest
/// is deleted.
pub fn clear(connection: &mut Connection) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        "DELETE FROM plays;
         DELETE FROM visits;
         DELETE FROM media
         WHERE id NOT IN (SELECT media_id FROM files WHERE media_id IS NOT NULL)
             AND id NOT IN (SELECT media_id FROM favorites WHERE media_id IS NOT NULL);
         UPDATE media SET position_ms = NULL, watched = 0, play_count = 0, last_played_ms = NULL;",
    )?;
    transaction.commit()
}

fn read_saved(row: &Row<'_>) -> rusqlite::Result<Saved> {
    let duration: Option<i64> = row.get(0)?;
    let position: Option<i64> = row.get(1)?;
    Ok(Saved {
        duration: duration.map(from_millis),
        position: position.map(from_millis),
        watched: row.get(2)?,
        play_count: row.get(3)?,
    })
}

fn stamp_columns(stamp: Option<FileStamp>) -> (Option<i64>, Option<i64>) {
    stamp.map_or((None, None), |stamp| (Some(stamp.size as i64), stamp.modified_ms))
}

fn millis(duration: Duration) -> i64 {
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

fn from_millis(millis: i64) -> Duration {
    Duration::from_millis(millis.max(0) as u64)
}
