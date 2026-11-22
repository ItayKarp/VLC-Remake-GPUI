//! Media rows as Home lists them (S4.9): what is partway watched (Continue Watching), what was
//! played lately (Listen Again), and single rows (favorite media), each with where its file is,
//! what the history remembers of it and its tags.
//!
//! These read the `media` row's own location, so media played from outside the library shows
//! too. Streams (rows with no file stamp) and orphaned rows don't.

use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension as _, Row, params};

use super::media::{self, MediaId, Saved};
use super::metadata::{self, MediaKind, Metadata};
use crate::settings::stored_path;

/// A media row with its file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Played {
    /// The media.
    pub media: MediaId,
    /// Where its file was last seen.
    pub path: PathBuf,
    /// What the history remembers of it.
    pub saved: Saved,
    /// Its tags, as far as the library read them.
    pub metadata: Metadata,
    /// When it joined the library, in milliseconds since the epoch, if known.
    pub added_ms: Option<i64>,
}

/// The columns [`read`] reads, after `SELECT`.
fn columns() -> String {
    format!(
        "media.duration_ms, media.position_ms, media.watched, media.play_count, media.id,
             media.location, media.added_ms, {}",
        metadata::COLUMNS
    )
}

/// What makes a row a file someone could play again.
const PLAYABLE: &str = "media.size IS NOT NULL AND media.orphaned_ms IS NULL";

/// Up to `limit` media that aren't audio, left partway and not watched, the most recently
/// played first; those taken off Home stay off until played again. Whether a position counts as
/// watched is the caller's rule.
pub fn in_progress(connection: &Connection, limit: usize) -> rusqlite::Result<Vec<Played>> {
    let mut statement = connection.prepare(&format!(
        "SELECT {columns} FROM media
         WHERE {PLAYABLE} AND media.kind != 'audio' AND media.watched = 0
             AND media.position_ms > 0
             AND (media.hidden_from_home_ms IS NULL
                 OR media.last_played_ms > media.hidden_from_home_ms)
         ORDER BY media.last_played_ms DESC LIMIT ?1",
        columns = columns()
    ))?;
    collect(statement.query_map([limit as i64], read)?)
}

/// Up to `limit` media of `kind` played most recently, each once.
pub fn recent(
    connection: &Connection,
    kind: MediaKind,
    limit: usize,
) -> rusqlite::Result<Vec<Played>> {
    let mut statement = connection.prepare(&format!(
        "SELECT {columns} FROM media
         JOIN (SELECT media_id, MAX(played_ms) AS last FROM plays GROUP BY media_id) AS latest
             ON latest.media_id = media.id
         WHERE {PLAYABLE} AND media.kind = ?1
         ORDER BY latest.last DESC LIMIT ?2",
        columns = columns()
    ))?;
    collect(statement.query_map(params![kind, limit as i64], read)?)
}

/// Media `id`, if it is a file.
pub fn one(connection: &Connection, id: MediaId) -> rusqlite::Result<Option<Played>> {
    let found = connection
        .query_row(
            &format!("SELECT {} FROM media WHERE media.id = ?1 AND {PLAYABLE}", columns()),
            [id.0],
            read,
        )
        .optional()?;
    Ok(found.flatten())
}

/// Takes media `id` off Continue Watching at `now_ms`, leaving its progress alone.
pub fn hide_in_progress(connection: &Connection, id: MediaId, now_ms: i64) -> rusqlite::Result<()> {
    connection.execute(
        "UPDATE media SET hidden_from_home_ms = ?2 WHERE id = ?1",
        params![id.0, now_ms],
    )?;
    Ok(())
}

fn collect(
    rows: impl Iterator<Item = rusqlite::Result<Option<Played>>>,
) -> rusqlite::Result<Vec<Played>> {
    let rows: Vec<Option<Played>> = rows.collect::<Result<_, _>>()?;
    Ok(rows.into_iter().flatten().collect())
}

/// A row of [`columns`], or `None` when its path was stored by another OS family.
fn read(row: &Row<'_>) -> rusqlite::Result<Option<Played>> {
    let saved = media::read_saved(row)?;
    let media = MediaId(row.get(4)?);
    let location: String = row.get(5)?;
    let added_ms = media::known_time(row.get(6)?);
    let metadata = metadata::read_from(row, 7)?;
    Ok(stored_path::decode(&location).map(|path| Played { media, path, saved, metadata, added_ms }))
}
