//! The indexer (S4.2): keeps the library's media folders indexed on a thread of its own, through
//! the history thread's database connection.
//!
//! - [`worker`]: the thread, which scans, follows the watcher and reconciles.
//! - [`scan`]: one scan of a folder, and identifying what it found.
//! - [`targets`]: what to rescan for a change the watcher saw.
//! - [`store`]: how the thread reaches the database.

mod scan;
pub mod store;
mod targets;
#[cfg(test)]
mod tests;
mod worker;

use std::path::PathBuf;

use super::progress::Progress;
use crate::db::roots::{Root, RootKind, RootStatus};

pub use worker::spawn;

/// What the indexer is asked to do.
#[derive(Debug)]
pub enum Command {
    /// Index exactly these media folders (the settings changed).
    SetRoots(Vec<Wanted>),
    /// Index hidden and system files too, or not (Settings → "Show hidden files").
    ShowHidden(bool),
    /// Rescan this folder and everything below it, reading every file again (S4.2.5).
    Resync(PathBuf),
    /// The watcher saw these paths change.
    Changed(Vec<PathBuf>),
}

/// A media folder the settings ask for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wanted {
    /// Where it is.
    pub path: PathBuf,
    /// What it holds.
    pub kind: RootKind,
}

/// A media folder as the UI shows it (S4.2.6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RootState {
    /// Where it is.
    pub path: PathBuf,
    /// What it holds.
    pub kind: RootKind,
    /// Whether it can be indexed.
    pub status: RootStatus,
    /// How many media files the library has in it.
    pub files: usize,
    /// How many of its folders couldn't be read, for the status details.
    pub unreadable: usize,
    /// How far the scan running in it has got, while one runs.
    pub indexing: Option<Progress>,
}

impl RootState {
    /// The state of `root` before anything is counted.
    pub fn of(root: &Root) -> Self {
        Self {
            path: root.path.clone(),
            kind: root.kind,
            status: root.status,
            files: 0,
            unreadable: 0,
            indexing: None,
        }
    }
}
