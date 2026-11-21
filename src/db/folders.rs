//! The `folders` table (S4.1): every folder of every media folder (root), once, whichever roots
//! reach it. Nested roots share their folders; a root's tree is its top folder and everything
//! below it through `parent_id`.
//!
//! Folders are found by [`PathKey`], so a path typed with other case or another Unicode form
//! finds the folder the file system would. The path is kept as first seen, for showing and
//! opening.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension as _, params};

use crate::library::path_key::{Case, PathKey};
use crate::settings::stored_path;

/// A row of the `folders` table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FolderId(pub i64);

/// A folder and where it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Folder {
    /// The row.
    pub id: FolderId,
    /// Its path, as first seen.
    pub path: PathBuf,
}

/// The folder at `path`, adding it below `parent` when it is new. A folder that had no parent
/// (it was a root's top) gets `parent` when a root above it is scanned.
pub fn ensure(
    connection: &Connection,
    path: &Path,
    parent: Option<FolderId>,
    case: Case,
) -> rusqlite::Result<FolderId> {
    connection.query_row(
        "INSERT INTO folders (parent_id, path, key) VALUES (?1, ?2, ?3)
         ON CONFLICT (key) DO UPDATE SET parent_id = COALESCE(parent_id, excluded.parent_id)
         RETURNING id",
        params![
            parent.map(|parent| parent.0),
            stored_path::encode(path),
            PathKey::new(path, case).as_str()
        ],
        |row| row.get(0).map(FolderId),
    )
}

/// The folder at `path`, if the library has it.
pub fn find(
    connection: &Connection,
    path: &Path,
    case: Case,
) -> rusqlite::Result<Option<FolderId>> {
    connection
        .query_row(
            "SELECT id FROM folders WHERE key = ?1",
            [PathKey::new(path, case).as_str()],
            |row| row.get(0).map(FolderId),
        )
        .optional()
}

/// The folders right below `id`, in path order.
pub fn children(connection: &Connection, id: FolderId) -> rusqlite::Result<Vec<Folder>> {
    let mut statement =
        connection.prepare("SELECT id, path FROM folders WHERE parent_id = ?1 ORDER BY key")?;
    let rows = statement.query_map([id.0], |row| Ok((FolderId(row.get(0)?), row.get(1)?)))?;
    let rows: Vec<(FolderId, String)> = rows.collect::<Result<_, _>>()?;
    Ok(rows
        .into_iter()
        .filter_map(|(id, path)| Some(Folder { id, path: decode(&path)? }))
        .collect())
}

/// Where folder `id` is, if it exists (and was stored on this OS family).
#[allow(dead_code, reason = "the library views (S4.6) open folders by id")]
pub fn path(connection: &Connection, id: FolderId) -> rusqlite::Result<Option<PathBuf>> {
    let stored: Option<String> = connection
        .query_row("SELECT path FROM folders WHERE id = ?1", [id.0], |row| row.get(0))
        .optional()?;
    Ok(stored.as_deref().and_then(decode))
}

/// Deletes folder `id` and everything below it, with their files: it is gone from the disk. A
/// root's top folder stays, with its files, since the root's own scans decide about it (a root
/// that went missing is marked so, not forgotten).
pub fn remove(connection: &Connection, id: FolderId) -> rusqlite::Result<()> {
    connection.execute(
        "DELETE FROM folders WHERE id = ?1 AND id NOT IN (SELECT folder_id FROM roots)",
        [id.0],
    )?;
    prune_unreachable(connection)?;
    Ok(())
}

/// Deletes the folders no root reaches any more, with their files.
pub(super) fn prune_unreachable(connection: &Connection) -> rusqlite::Result<usize> {
    connection.execute(
        "WITH RECURSIVE kept (id) AS (
             SELECT folder_id FROM roots
             UNION SELECT folders.id FROM folders JOIN kept ON folders.parent_id = kept.id
         )
         DELETE FROM folders WHERE id NOT IN kept",
        [],
    )
}

/// A stored path, or `None` for one stored by another OS family.
pub(super) fn decode(stored: &str) -> Option<PathBuf> {
    stored_path::decode(stored)
}
