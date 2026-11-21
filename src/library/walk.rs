//! Walking a media folder for the indexer (S4.2.2): a few threads list folders in parallel and
//! stream each folder's media files, as a [`Listing`], to whoever indexes them.
//!
//! - The pool is small ([`MAX_WORKERS`]), so antivirus scanners and network shares aren't
//!   hammered, and the channel is bounded, so memory stays flat however large the tree is.
//! - A folder's listing is sent before its subfolders are listed, so the indexer always knows a
//!   folder's parent first.
//! - Each folder is listed once, by file identity, so symlink and junction loops end.
//! - Hidden and system files are skipped unless asked for; a folder that can't be read is
//!   reported and skipped; a file deleted meanwhile is simply not listed.

use std::collections::HashSet;
use std::ffi::OsString;
use std::fs::{self, DirEntry, Metadata};
use std::io;
use std::num::NonZero;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::{Condvar, Mutex, PoisonError};
use std::thread;

use super::{placeholder, recognize};
use crate::db::media::FileStamp;
use crate::history::identity;
use crate::opening::Cancel;
use crate::opening::scan::is_hidden;

/// The most threads one walk lists folders on.
pub const MAX_WORKERS: usize = 4;

/// A media file a walk found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    /// Its name in its folder.
    pub name: OsString,
    /// Its size and modification time.
    pub stamp: FileStamp,
    /// Whether it is online-only, so its content mustn't be read.
    pub cloud: bool,
}

/// One folder as a walk saw it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listing {
    /// The folder.
    pub dir: PathBuf,
    /// Its media files.
    pub files: Vec<Found>,
    /// The names of its subfolders (each listed once per walk, whatever links lead to it).
    pub subdirs: Vec<OsString>,
}

/// What a walk reports.
#[derive(Debug)]
pub enum Walked {
    /// A folder was listed.
    Listed(Listing),
    /// A folder couldn't be read.
    Unreadable(PathBuf, io::ErrorKind),
}

/// How far below the folder it starts at a walk goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Depth {
    /// Only that folder (a change inside it).
    Shallow,
    /// That folder and everything below it.
    Tree,
}

/// What a walk lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    /// How deep to go.
    pub depth: Depth,
    /// Whether hidden and system files and folders are listed (Settings → "Show hidden files").
    pub show_hidden: bool,
}

/// How far a walk has got, for the progress estimate.
#[derive(Debug, Default)]
pub struct Counters {
    /// Folders found so far, the top one included.
    pub found: AtomicUsize,
    /// Folders listed (or found unreadable) so far.
    pub listed: AtomicUsize,
}

/// Walks from `top`, sending what it finds to `sink`, and returns when done or cancelled.
pub fn walk(
    top: &Path,
    options: Options,
    cancel: &Cancel,
    counters: &Counters,
    sink: &SyncSender<Walked>,
) {
    let walk = Walk { options, cancel, counters, queue: Queue::new(top), seen: Mutex::default() };
    if let Ok(metadata) = fs::metadata(top) {
        walk.first_visit(top, &metadata);
    }
    counters.found.fetch_add(1, Ordering::Relaxed);
    let workers = thread::available_parallelism().map_or(1, NonZero::get).min(MAX_WORKERS);
    thread::scope(|scope| {
        for _ in 0..workers {
            let sink = sink.clone();
            let walk = &walk;
            scope.spawn(move || walk.work(&sink));
        }
    });
}

struct Walk<'a> {
    options: Options,
    cancel: &'a Cancel,
    counters: &'a Counters,
    queue: Queue,
    /// The folders already found, by identity.
    seen: Mutex<HashSet<DirKey>>,
}

impl Walk<'_> {
    fn work(&self, sink: &SyncSender<Walked>) {
        while let Some(dir) = self.queue.next(self.cancel) {
            let subdirs = match self.list(&dir) {
                Ok(listing) => self.send(listing, sink),
                Err(error) => {
                    let _ = sink.send(Walked::Unreadable(dir, error.kind()));
                    Vec::new()
                }
            };
            self.counters.listed.fetch_add(1, Ordering::Relaxed);
            self.counters.found.fetch_add(subdirs.len(), Ordering::Relaxed);
            self.queue.done(subdirs);
        }
    }

    /// Sends `listing`, and answers the subfolders to list next.
    fn send(&self, listing: Listing, sink: &SyncSender<Walked>) -> Vec<PathBuf> {
        let subdirs = self.to_descend(&listing);
        // The indexer is gone (the app is quitting): stop.
        if sink.send(Walked::Listed(listing)).is_err() {
            self.cancel.cancel();
        }
        subdirs
    }

    fn list(&self, dir: &Path) -> io::Result<Listing> {
        let mut listing =
            Listing { dir: dir.to_path_buf(), files: Vec::new(), subdirs: Vec::new() };
        for entry in fs::read_dir(dir)?.flatten() {
            if self.cancel.is_cancelled() {
                break;
            }
            self.consider(&entry, &mut listing);
        }
        Ok(listing)
    }

    fn consider(&self, entry: &DirEntry, listing: &mut Listing) {
        if !self.options.show_hidden && is_hidden(entry) {
            return;
        }
        let path = entry.path();
        // Follows links, so a link to a file counts as that file. Fails for a file deleted
        // since the folder was read, or a broken link: neither is there to index.
        let Ok(metadata) = fs::metadata(&path) else { return };
        if metadata.is_dir() {
            if self.first_visit(&path, &metadata) {
                listing.subdirs.push(entry.file_name());
            }
        } else if metadata.is_file() {
            let cloud = placeholder::is_online_only(&metadata);
            if recognize::is_media(&path, cloud) {
                let stamp = identity::stamp(&metadata);
                listing.files.push(Found { name: entry.file_name(), stamp, cloud });
            }
        }
    }

    /// The subfolders of `listing` to list next, if the walk goes below it.
    fn to_descend(&self, listing: &Listing) -> Vec<PathBuf> {
        match self.options.depth {
            Depth::Shallow => Vec::new(),
            Depth::Tree => listing.subdirs.iter().map(|name| listing.dir.join(name)).collect(),
        }
    }

    /// Whether the folder at `path` wasn't found before in this walk (by any other path).
    fn first_visit(&self, path: &Path, metadata: &Metadata) -> bool {
        let Some(key) = dir_key(path, metadata) else { return true };
        self.seen.lock().unwrap_or_else(PoisonError::into_inner).insert(key)
    }
}

/// The folders waiting to be listed, and how many are being listed now: the walk is over when
/// both are none.
struct Queue {
    state: Mutex<QueueState>,
    changed: Condvar,
}

struct QueueState {
    waiting: Vec<PathBuf>,
    listing: usize,
}

impl Queue {
    fn new(top: &Path) -> Self {
        let state = QueueState { waiting: vec![top.to_path_buf()], listing: 0 };
        Self { state: Mutex::new(state), changed: Condvar::new() }
    }

    /// The next folder to list; `None` once there is nothing left (or the walk was cancelled).
    fn next(&self, cancel: &Cancel) -> Option<PathBuf> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            if cancel.is_cancelled() {
                state.waiting.clear();
            }
            if let Some(dir) = state.waiting.pop() {
                state.listing += 1;
                return Some(dir);
            }
            if state.listing == 0 {
                return None;
            }
            state = self.changed.wait(state).unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// A folder was listed, and these are its subfolders to list.
    fn done(&self, subdirs: Vec<PathBuf>) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.waiting.extend(subdirs);
        state.listing -= 1;
        self.changed.notify_all();
    }
}

/// What tells two paths to one folder apart from two folders.
#[cfg(unix)]
type DirKey = (u64, u64);

#[cfg(unix)]
#[allow(clippy::unnecessary_wraps, reason = "Windows can fail to resolve the folder")]
fn dir_key(_: &Path, metadata: &Metadata) -> Option<DirKey> {
    use std::os::unix::fs::MetadataExt as _;
    Some((metadata.dev(), metadata.ino()))
}

/// What tells two paths to one folder apart from two folders: `std` has no stable file id on
/// Windows, so the path with links and junctions resolved.
#[cfg(not(unix))]
type DirKey = PathBuf;

#[cfg(not(unix))]
fn dir_key(path: &Path, _: &Metadata) -> Option<DirKey> {
    fs::canonicalize(path).ok()
}

#[cfg(test)]
mod tests;
