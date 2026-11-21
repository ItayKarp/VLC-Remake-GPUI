//! The `files` table (S4.1): each media file the library found, by folder and name, and the
//! `media` row its content is. One media row can have several files: the same file reached
//! through a symlink, or copies in two places, show in each tree but share their history.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, Row, params};

use super::folders::{self, FolderId};
use super::media::{FileStamp, MediaId};
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
        "INSERT INTO files (folder_id, name, key, media_id, size, modified_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT (folder_id, key) DO UPDATE SET name = excluded.name,
             media_id = excluded.media_id, size = excluded.size, modified_ms = excluded.modified_ms
         RETURNING id",
        params![
            folder.0,
            stored_path::encode(name),
            PathKey::new(name, case).as_str(),
            file.media.map(|media| media.0),
            file.stamp.size as i64,
            file.stamp.modified_ms
        ],
        |row| row.get(0).map(FileId),
    )
}

/// The files in `folder`, in name order.
pub fn in_folder(connection: &Connection, folder: FolderId) -> rusqlite::Result<Vec<File>> {
    select(connection, "files.folder_id = ?1 ORDER BY files.key", folder.0)
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

fn select(connection: &Connection, filter: &str, value: i64) -> rusqlite::Result<Vec<File>> {
    let mut statement = connection.prepare(&format!(
        "SELECT files.id, folders.path, files.name, files.size, files.modified_ms, files.media_id
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
    let path = folders::decode(&folder).zip(stored_path::decode(&name));
    Ok(path.map(|(folder, name)| File { id, path: folder.join(name), stamp, media }))
}
