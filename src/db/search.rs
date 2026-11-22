//! The library's search index (S4.7.5): an FTS5 table holding, for each file, its name and its
//! media's title, artist, album artist and album, kept up to date by triggers.
//!
//! The trigram tokenizer matches any part of a word, which is what typing into a search field
//! expects and what text without spaces (Chinese, Japanese) needs; it ignores case and, with
//! `remove_diacritics`, accents. A trigram needs three characters, so shorter words are matched
//! by the caller ([`crate::library::search`]) against the text each hit returns.

use std::path::PathBuf;

use rusqlite::{Connection, Row};

use super::folders;
use super::media::{self, MediaId, Saved};
use super::metadata::{self, Metadata};
use crate::settings::stored_path;

/// Version 7 (S4.7.5): the search index, filled from the files already there.
pub(super) const MIGRATION: &str = "
    CREATE VIRTUAL TABLE search USING fts5 (text, tokenize = 'trigram remove_diacritics 1');
    INSERT INTO search (rowid, text)
        SELECT files.id, files.name || ' ' || COALESCE(media.title, '') || ' '
            || COALESCE(media.artist, '') || ' ' || COALESCE(media.album_artist, '') || ' '
            || COALESCE(media.album, '')
        FROM files LEFT JOIN media ON media.id = files.media_id;
    CREATE TRIGGER search_file_added AFTER INSERT ON files BEGIN
        INSERT INTO search (rowid, text)
            SELECT new.id, new.name || ' ' || COALESCE(media.title, '') || ' '
                || COALESCE(media.artist, '') || ' ' || COALESCE(media.album_artist, '') || ' '
                || COALESCE(media.album, '')
            FROM (SELECT 1) LEFT JOIN media ON media.id = new.media_id;
    END;
    CREATE TRIGGER search_file_changed AFTER UPDATE OF name, media_id ON files BEGIN
        DELETE FROM search WHERE rowid = old.id;
        INSERT INTO search (rowid, text)
            SELECT new.id, new.name || ' ' || COALESCE(media.title, '') || ' '
                || COALESCE(media.artist, '') || ' ' || COALESCE(media.album_artist, '') || ' '
                || COALESCE(media.album, '')
            FROM (SELECT 1) LEFT JOIN media ON media.id = new.media_id;
    END;
    CREATE TRIGGER search_file_removed AFTER DELETE ON files BEGIN
        DELETE FROM search WHERE rowid = old.id;
    END;
    CREATE TRIGGER search_tags_changed AFTER UPDATE OF title, artist, album_artist, album ON media
    BEGIN
        DELETE FROM search WHERE rowid IN (SELECT id FROM files WHERE media_id = new.id);
        INSERT INTO search (rowid, text)
            SELECT files.id, files.name || ' ' || COALESCE(new.title, '') || ' '
                || COALESCE(new.artist, '') || ' ' || COALESCE(new.album_artist, '') || ' '
                || COALESCE(new.album, '')
            FROM files WHERE files.media_id = new.id;
    END;
";

/// One file the index found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    /// Where it is.
    pub path: PathBuf,
    /// The text the index holds for it: its name, title, artists and album.
    pub text: String,
    /// The media its content is; `None` until identified.
    pub media: Option<MediaId>,
    /// What the history remembers of it.
    pub saved: Saved,
    /// Its kind and tags, as far as they were read.
    pub metadata: Metadata,
    /// When its media joined the library, in milliseconds since the epoch, if known.
    pub added_ms: Option<i64>,
}

/// The files whose text matches `expression` (FTS5 query syntax), or every file when it is
/// `None`, at most `limit` of them, in no particular order.
pub fn find(
    connection: &Connection,
    expression: Option<&str>,
    limit: usize,
) -> rusqlite::Result<Vec<Hit>> {
    let filter = if expression.is_some() { "search MATCH ?1" } else { "?1 IS NULL" };
    let mut statement = connection.prepare(&format!(
        "SELECT media.duration_ms, media.position_ms, COALESCE(media.watched, 0),
             COALESCE(media.play_count, 0), files.media_id, media.added_ms, folders.path,
             files.name, search.text, {columns}
         FROM search JOIN files ON files.id = search.rowid
             JOIN folders ON folders.id = files.folder_id
             LEFT JOIN media ON media.id = files.media_id
         WHERE {filter} LIMIT ?2",
        // A file not identified yet has no media row, so no kind either.
        columns = metadata::COLUMNS.replacen("media.kind", "COALESCE(media.kind, 'unknown')", 1)
    ))?;
    let rows = statement.query_map(rusqlite::params![expression, limit as i64], read)?;
    let rows: Vec<Option<Hit>> = rows.collect::<Result<_, _>>()?;
    Ok(rows.into_iter().flatten().collect())
}

/// A hit, or `None` when its path was stored by another OS family.
fn read(row: &Row<'_>) -> rusqlite::Result<Option<Hit>> {
    let saved = media::read_saved(row)?;
    let media = row.get::<_, Option<i64>>(4)?.map(MediaId);
    let added_ms = media::known_time(row.get(5)?);
    let (folder, name): (String, String) = (row.get(6)?, row.get(7)?);
    let text = row.get(8)?;
    let metadata = metadata::read_from(row, 9)?;
    let path = folders::decode(&folder).zip(stored_path::decode(&name));
    Ok(path.map(|(folder, name)| Hit {
        path: folder.join(name),
        text,
        media,
        saved,
        metadata,
        added_ms,
    }))
}
