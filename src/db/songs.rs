//! The library's songs (S4.6.2): every audio media the library has a file of, with its tags and
//! history, for Music's Albums, Artists and Genres.

use std::path::PathBuf;

use rusqlite::{Connection, Row};

use super::folders;
use super::media::{self, MediaId, Saved};
use super::metadata::{self, Metadata};
use crate::settings::stored_path;

/// One song: its media, one of its files, its tags and its history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Song {
    /// The media.
    pub media: MediaId,
    /// One of its files (the first the library found), to play.
    pub path: PathBuf,
    /// Its tags.
    pub metadata: Metadata,
    /// What the history remembers of it.
    pub saved: Saved,
}

/// Every song, in no order. A song with files in several places is listed once.
pub fn all(connection: &Connection) -> rusqlite::Result<Vec<Song>> {
    // With `MIN`, SQLite takes the other columns from the row holding the minimum: the first
    // file found.
    let mut statement = connection.prepare(&format!(
        "SELECT media.duration_ms, media.position_ms, media.watched, media.play_count,
             media.id, folders.path, files.name, {columns}, MIN(files.id)
         FROM media JOIN files ON files.media_id = media.id
             JOIN folders ON folders.id = files.folder_id
         WHERE media.kind = 'audio' AND media.orphaned_ms IS NULL
         GROUP BY media.id",
        columns = metadata::COLUMNS
    ))?;
    let rows = statement.query_map([], read)?;
    let rows: Vec<Option<Song>> = rows.collect::<Result<_, _>>()?;
    Ok(rows.into_iter().flatten().collect())
}

/// A song row, or `None` when its path was stored by another OS family.
fn read(row: &Row<'_>) -> rusqlite::Result<Option<Song>> {
    let saved = media::read_saved(row)?;
    let media = MediaId(row.get(4)?);
    let (folder, name): (String, String) = (row.get(5)?, row.get(6)?);
    let metadata = metadata::read_from(row, 7)?;
    let path = folders::decode(&folder).zip(stored_path::decode(&name));
    Ok(path.map(|(folder, name)| Song { media, path: folder.join(name), metadata, saved }))
}
