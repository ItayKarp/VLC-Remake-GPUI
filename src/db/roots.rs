//! The `roots` table (S4.1): the media folders the library indexes, what they hold and whether
//! they can be reached. A root is a row pointing at its top folder in [`folders`]; nested roots
//! share the folders below the inner one.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, Row, params};

use super::folders::{self, FolderId};
use super::text_enum::text_enum;
use crate::library::path_key::Case;
use crate::settings::stored_path;

/// A row of the `roots` table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RootId(pub i64);

/// What a media folder holds, which decides the library section it feeds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootKind {
    /// Videos (the onboarding folder).
    Video,
    /// Music (the OS Music folder, S4.2.7).
    Music,
    /// Both: each file goes to the section its kind belongs to.
    Mixed,
}

text_enum!(RootKind { Video => "video", Music => "music", Mixed => "mixed" });

/// Whether a media folder can be indexed now (S4.2.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootStatus {
    /// It can be read.
    Ok,
    /// It's gone: deleted, renamed, or on a drive that isn't plugged in.
    Missing,
    /// It exists but can't be read.
    PermissionDenied,
    /// It's on a network share that answers slowly.
    Slow,
}

text_enum!(RootStatus {
    Ok => "ok",
    Missing => "missing",
    PermissionDenied => "permission_denied",
    Slow => "slow",
});

/// A media folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Root {
    /// The row.
    pub id: RootId,
    /// Its top folder.
    pub folder: FolderId,
    /// Where it is.
    pub path: PathBuf,
    /// What it holds.
    pub kind: RootKind,
    /// Whether it can be indexed.
    pub status: RootStatus,
    /// How its file system compares names.
    pub case: Case,
}

/// Adds the media folder at `path`, or returns the one already there. A root inside another
/// root hangs its top folder below the outer root's folder, so both trees share it.
pub fn add(
    connection: &mut Connection,
    path: &Path,
    kind: RootKind,
    case: Case,
    now_ms: i64,
) -> rusqlite::Result<RootId> {
    let transaction = connection.transaction()?;
    let parent = match path.parent() {
        Some(parent) => folders::find(&transaction, parent, case)?,
        None => None,
    };
    let folder = folders::ensure(&transaction, path, parent, case)?;
    let id = transaction.query_row(
        // The no-op update makes `RETURNING` give the existing root's id; it keeps its kind.
        "INSERT INTO roots (folder_id, kind, case_sensitive, added_ms) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (folder_id) DO UPDATE SET folder_id = folder_id
         RETURNING id",
        params![folder.0, kind, case.is_sensitive(), now_ms],
        |row| row.get(0).map(RootId),
    )?;
    transaction.commit()?;
    Ok(id)
}

/// Every media folder, oldest first (the onboarding folder leads).
pub fn list(connection: &Connection) -> rusqlite::Result<Vec<Root>> {
    let mut statement = connection.prepare(
        "SELECT roots.id, folder_id, path, kind, status, case_sensitive
         FROM roots JOIN folders ON folders.id = roots.folder_id ORDER BY roots.id",
    )?;
    let rows = statement.query_map([], read)?;
    let rows: Vec<Option<Root>> = rows.collect::<Result<_, _>>()?;
    Ok(rows.into_iter().flatten().collect())
}

/// Records whether root `id` can be indexed.
pub fn set_status(connection: &Connection, id: RootId, status: RootStatus) -> rusqlite::Result<()> {
    connection.execute("UPDATE roots SET status = ?2 WHERE id = ?1", params![id.0, status])?;
    Ok(())
}

/// Records that root `id` was scanned through at `now_ms`.
pub fn set_scanned(connection: &Connection, id: RootId, now_ms: i64) -> rusqlite::Result<()> {
    connection.execute("UPDATE roots SET scanned_ms = ?2 WHERE id = ?1", params![id.0, now_ms])?;
    Ok(())
}

/// Replaces the folders a scan of root `id` couldn't read.
pub fn set_unreadable(
    connection: &mut Connection,
    id: RootId,
    paths: &[PathBuf],
) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute("DELETE FROM unreadable_folders WHERE root_id = ?1", [id.0])?;
    for path in paths {
        transaction.execute(
            "INSERT OR IGNORE INTO unreadable_folders (root_id, path) VALUES (?1, ?2)",
            params![id.0, stored_path::encode(path)],
        )?;
    }
    transaction.commit()
}

/// The folders the last scan of root `id` couldn't read, in path order.
pub fn unreadable(connection: &Connection, id: RootId) -> rusqlite::Result<Vec<PathBuf>> {
    let mut statement = connection
        .prepare("SELECT path FROM unreadable_folders WHERE root_id = ?1 ORDER BY path")?;
    let rows = statement.query_map([id.0], |row| row.get::<_, String>(0))?;
    let rows: Vec<String> = rows.collect::<Result<_, _>>()?;
    Ok(rows.iter().filter_map(|path| folders::decode(path)).collect())
}

/// Removes root `id`, and the folders and files no other root reaches. The media rows stay:
/// their history isn't the library's to forget.
pub fn remove(connection: &mut Connection, id: RootId) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute("DELETE FROM roots WHERE id = ?1", [id.0])?;
    folders::prune_unreachable(&transaction)?;
    transaction.commit()
}

/// A root row, or `None` when its path was stored by another OS family.
fn read(row: &Row<'_>) -> rusqlite::Result<Option<Root>> {
    let id = RootId(row.get(0)?);
    let folder = FolderId(row.get(1)?);
    let path: String = row.get(2)?;
    let (kind, status) = (row.get(3)?, row.get(4)?);
    let case = Case::from_sensitive(row.get(5)?);
    Ok(folders::decode(&path).map(|path| Root { id, folder, path, kind, status, case }))
}
