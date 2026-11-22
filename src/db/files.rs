//! The `files` table (S4.1): each media file the library found, by folder and name, and the
//! `media` row its content is. One media row can have several files: the same file reached
//! through a symlink, or copies in two places, show in each tree but share their history.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, Row, params};

use super::folders::{self, FolderId};
use super::media::{self, FileStamp, MediaId, Saved};
use super::metadata::{self, Metadata};
use crate::library::path_key::{Case, PathKey};
use crate::settings::stored_path;

/// A row of the `files` table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FileId(pub i64);

/// What a scan saw of a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scanned<'a> {
    /// The file's name in its folder.
    pub name: &'a OsStr,
    /// Its size and modification time.
    pub stamp: FileStamp,
    /// The media its content is, once identified.
    pub media: Option<MediaId>,
    /// Whether it is an online-only placeholder, whose content mustn't be read.
    pub cloud: bool,
}

/// A file in the library.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    /// The row.
    pub id: FileId,
    /// Where it is.
    pub path: PathBuf,
    /// Its size and modification time when last scanned.
    pub stamp: FileStamp,
    /// The media its content is; `None` until identified.
    pub media: Option<MediaId>,
    /// Whether it is an online-only placeholder.
    pub cloud: bool,
}

/// A file waiting to be identified.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unidentified {
    /// The row.
    pub id: FileId,
    /// Where it is.
    pub path: PathBuf,
    /// Its size and modification time when last scanned.
    pub stamp: FileStamp,
}

/// A file in a folder as the library views list it (S4.6): its name, and what the library knows
/// of its media.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
    /// The file's name in its folder.
    pub name: PathBuf,
    /// The media its content is; `None` until identified.
    pub media: Option<MediaId>,
    /// What the history remembers of it.
    pub saved: Saved,
    /// Its kind (video or audio, once probed) and tags, once read (S4.3).
    pub metadata: Metadata,
    /// When its media joined the library, in milliseconds since the epoch, if known.
    pub added_ms: Option<i64>,
}

/// Writes what a scan saw of a file in `folder`, adding it or updating the one there.
pub fn put(
    connection: &Connection,
    folder: FolderId,
    file: &Scanned<'_>,
    case: Case,
) -> rusqlite::Result<FileId> {
    let name = Path::new(file.name);
    connection.query_row(
        "INSERT INTO files (folder_id, name, key, media_id, size, modified_ms, cloud)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT (folder_id, key) DO UPDATE SET name = excluded.name,
             media_id = excluded.media_id, size = excluded.size,
             modified_ms = excluded.modified_ms, cloud = excluded.cloud
         RETURNING id",
        params![
            folder.0,
            stored_path::encode(name),
            PathKey::new(name, case).as_str(),
            file.media.map(|media| media.0),
            file.stamp.size as i64,
            file.stamp.modified_ms,
            file.cloud
        ],
        |row| row.get(0).map(FileId),
    )
}

/// The files in `folder`, in name order.
pub fn in_folder(connection: &Connection, folder: FolderId) -> rusqlite::Result<Vec<File>> {
    select(connection, "files.folder_id = ?1 ORDER BY files.key", folder.0)
}

/// The files in `folder`, with their media's kind and history, for the views.
pub fn listed(connection: &Connection, folder: FolderId) -> rusqlite::Result<Vec<Listed>> {
    let mut statement = connection.prepare(&format!(
        "SELECT media.duration_ms, media.position_ms, COALESCE(media.watched, 0),
             COALESCE(media.play_count, 0), files.name, files.media_id, media.added_ms, {columns}
         FROM files LEFT JOIN media ON media.id = files.media_id
         WHERE files.folder_id = ?1",
        // A file not identified yet has no media row, so no kind either.
        columns = metadata::COLUMNS.replacen("media.kind", "COALESCE(media.kind, 'unknown')", 1)
    ))?;
    let rows = statement.query_map([folder.0], read_listed)?;
    let rows: Vec<Option<Listed>> = rows.collect::<Result<_, _>>()?;
    Ok(rows.into_iter().flatten().collect())
}

/// A row of [`listed`], or `None` when its name was stored by another OS family.
fn read_listed(row: &Row<'_>) -> rusqlite::Result<Option<Listed>> {
    let saved = media::read_saved(row)?;
    let name: String = row.get(4)?;
    let media = row.get::<_, Option<i64>>(5)?.map(MediaId);
    let added_ms = media::known_time(row.get(6)?);
    let metadata = metadata::read_from(row, 7)?;
    Ok(stored_path::decode(&name).map(|name| Listed { name, media, saved, metadata, added_ms }))
}

/// Every file whose content is `media`: each place the library shows it.
pub fn of_media(connection: &Connection, media: MediaId) -> rusqlite::Result<Vec<File>> {
    select(connection, "files.media_id = ?1 ORDER BY folders.key, files.key", media.0)
}

/// Forgets file `id`.
pub fn remove(connection: &Connection, id: FileId) -> rusqlite::Result<()> {
    connection.execute("DELETE FROM files WHERE id = ?1", [id.0])?;
    Ok(())
}

/// Records that file `id`'s content is `media`, as identified when the file had `stamp`. A file
/// that changed since (a later scan saw another stamp) is left for identifying again.
pub fn set_media(
    connection: &Connection,
    id: FileId,
    media: MediaId,
    stamp: FileStamp,
) -> rusqlite::Result<()> {
    connection.execute(
        "UPDATE files SET media_id = ?2 WHERE id = ?1 AND size = ?3 AND modified_ms IS ?4",
        params![id.0, media.0, stamp.size as i64, stamp.modified_ms],
    )?;
    Ok(())
}

/// Up to `limit` files after `after` (by row) whose content hasn't been identified, leaving out
/// online-only ones: the identifier's next batch.
pub fn unidentified(
    connection: &Connection,
    after: FileId,
    limit: usize,
) -> rusqlite::Result<Vec<Unidentified>> {
    let mut statement = connection.prepare(
        "SELECT files.id, folders.path, files.name, files.size, files.modified_ms
         FROM files JOIN folders ON folders.id = files.folder_id
         WHERE files.media_id IS NULL AND files.cloud = 0 AND files.id > ?1
         ORDER BY files.id LIMIT ?2",
    )?;
    let rows = statement.query_map(params![after.0, limit as i64], |row| {
        let (folder, name): (String, String) = (row.get(1)?, row.get(2)?);
        let size: i64 = row.get(3)?;
        let stamp = FileStamp { size: size as u64, modified_ms: row.get(4)? };
        Ok((FileId(row.get(0)?), folders::decode(&folder).zip(stored_path::decode(&name)), stamp))
    })?;
    let rows: Vec<_> = rows.collect::<Result<_, _>>()?;
    Ok(rows
        .into_iter()
        .filter_map(|(id, path, stamp)| {
            path.map(|(folder, name)| Unidentified { id, path: folder.join(name), stamp })
        })
        .collect())
}

/// How many files are in folder `top` and every folder below it.
pub fn count_below(connection: &Connection, top: FolderId) -> rusqlite::Result<usize> {
    connection.query_row(
        "WITH RECURSIVE tree (id) AS (
             SELECT ?1 UNION SELECT folders.id FROM folders JOIN tree ON folders.parent_id = tree.id
         )
         SELECT COUNT(*) FROM files WHERE folder_id IN tree",
        [top.0],
        |row| row.get::<_, i64>(0).map(|count| count as usize),
    )
}

fn select(connection: &Connection, filter: &str, value: i64) -> rusqlite::Result<Vec<File>> {
    let mut statement = connection.prepare(&format!(
        "SELECT files.id, folders.path, files.name, files.size, files.modified_ms, files.media_id,
             files.cloud
         FROM files JOIN folders ON folders.id = files.folder_id WHERE {filter}"
    ))?;
    let rows = statement.query_map(params![value], read)?;
    let rows: Vec<Option<File>> = rows.collect::<Result<_, _>>()?;
    Ok(rows.into_iter().flatten().collect())
}

/// A file row, or `None` when its path was stored by another OS family.
fn read(row: &Row<'_>) -> rusqlite::Result<Option<File>> {
    let id = FileId(row.get(0)?);
    let (folder, name): (String, String) = (row.get(1)?, row.get(2)?);
    let size: i64 = row.get(3)?;
    let stamp = FileStamp { size: size as u64, modified_ms: row.get(4)? };
    let media = row.get::<_, Option<i64>>(5)?.map(MediaId);
    let cloud = row.get(6)?;
    let path = folders::decode(&folder).zip(stored_path::decode(&name));
    Ok(path.map(|(folder, name)| File { id, path: folder.join(name), stamp, media, cloud }))
}
