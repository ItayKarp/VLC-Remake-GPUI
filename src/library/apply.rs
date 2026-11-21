//! Writing what a walk saw of a folder into the library (S4.2.2, S4.2.3): new files are added,
//! files whose size or modification time changed are marked for identifying again, files and
//! subfolders that are gone are removed. A file whose stamp didn't change keeps its media row
//! and isn't read again, which is what makes a rescan cheap.
//!
//! Only a folder that was listed is changed: a folder that couldn't be read, or a drive unplugged
//! halfway through a scan, keeps what the library had.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use super::path_key::{Case, PathKey};
use super::walk::{Found, Listing};
use crate::db::files::{self, File, Scanned};
use crate::db::folders::{self, FolderId};

/// What writing one listing found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Applied {
    /// Files added, or changed since the last scan.
    pub changed: usize,
    /// Files removed.
    pub removed: usize,
    /// Subfolders the library didn't have, which a shallow scan should walk too.
    pub new_subdirs: Vec<PathBuf>,
}

/// Which files a rescan identifies again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Rescan {
    /// Only new files and those whose size or modification time changed.
    #[default]
    Changed,
    /// Every file (Resync, S4.2.5).
    Everything,
}

/// Writes `listings` in one transaction, adding up what they changed.
pub fn apply_all(
    connection: &mut Connection,
    listings: &[Listing],
    case: Case,
    rescan: Rescan,
) -> rusqlite::Result<Applied> {
    let transaction = connection.transaction()?;
    let mut total = Applied::default();
    for listing in listings {
        let applied = apply(&transaction, listing, case, rescan)?;
        total.changed += applied.changed;
        total.removed += applied.removed;
        total.new_subdirs.extend(applied.new_subdirs);
    }
    transaction.commit()?;
    Ok(total)
}

/// Writes one listing.
pub fn apply(
    connection: &Connection,
    listing: &Listing,
    case: Case,
    rescan: Rescan,
) -> rusqlite::Result<Applied> {
    let parent = match listing.dir.parent() {
        Some(parent) => folders::find(connection, parent, case)?,
        None => None,
    };
    let folder = folders::ensure(connection, &listing.dir, parent, case)?;
    let (changed, removed) = apply_files(connection, folder, &listing.files, (case, rescan))?;
    let new_subdirs = apply_subdirs(connection, folder, listing, case)?;
    Ok(Applied { changed, removed, new_subdirs })
}

/// Brings the files of `folder` in line with `found`: how many changed and how many went.
fn apply_files(
    connection: &Connection,
    folder: FolderId,
    found: &[Found],
    (case, rescan): (Case, Rescan),
) -> rusqlite::Result<(usize, usize)> {
    let mut known: HashMap<PathKey, File> = files::in_folder(connection, folder)?
        .into_iter()
        .map(|file| (name_key(&file.path, case), file))
        .collect();
    let mut changed = 0;
    for file in found {
        let before = known.remove(&PathKey::new(Path::new(&file.name), case));
        let unchanged = before.as_ref().is_some_and(|before| is_unchanged(before, file));
        if unchanged && rescan == Rescan::Changed {
            continue;
        }
        // A new or changed file is identified again: its content may be other media now.
        let scanned =
            Scanned { name: &file.name, stamp: file.stamp, media: None, cloud: file.cloud };
        files::put(connection, folder, &scanned, case)?;
        changed += 1;
    }
    for gone in known.values() {
        files::remove(connection, gone.id)?;
    }
    Ok((changed, known.len()))
}

/// Removes the subfolders of `folder` that are gone, and names the ones that are new.
fn apply_subdirs(
    connection: &Connection,
    folder: FolderId,
    listing: &Listing,
    case: Case,
) -> rusqlite::Result<Vec<PathBuf>> {
    let present: HashSet<PathKey> =
        listing.subdirs.iter().map(|name| PathKey::new(Path::new(name), case)).collect();
    let mut known = HashSet::new();
    for child in folders::children(connection, folder)? {
        let key = name_key(&child.path, case);
        if present.contains(&key) {
            known.insert(key);
        } else {
            folders::remove(connection, child.id)?;
        }
    }
    let new = listing.subdirs.iter().filter(|name| {
        let key = PathKey::new(Path::new(name), case);
        !known.contains(&key)
    });
    Ok(new.map(|name| listing.dir.join(name)).collect())
}

/// A file is unchanged when its size, modification time and online-only state are. One that
/// wasn't identified yet stays in the identifier's queue either way.
fn is_unchanged(before: &File, now: &Found) -> bool {
    before.stamp == now.stamp && before.cloud == now.cloud
}

fn name_key(path: &Path, case: Case) -> PathKey {
    PathKey::new(Path::new(path.file_name().unwrap_or_default()), case)
}

#[cfg(test)]
mod tests;
