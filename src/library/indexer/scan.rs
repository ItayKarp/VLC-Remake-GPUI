//! One scan: walk a folder of a media folder, write what was found in batches, and identify the
//! files that are new or changed (S4.2.2, S4.2.3).

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use super::store::{Store, call};
use crate::db::files::{self, FileId};
use crate::db::folders;
use crate::db::now_ms;
use crate::db::roots::Root;
use crate::library::apply::{self, Rescan};
use crate::library::identify;
use crate::library::progress::{Progress, Sample};
use crate::library::walk::{self, Counters, Depth, Listing, Walked};
use crate::opening::Cancel;

/// How many listed files are written in one transaction.
const BATCH_FILES: usize = 500;
/// How many listings, however small, are written in one transaction.
const BATCH_LISTINGS: usize = 64;
/// How many listings may wait to be written: bounds the memory a huge tree takes.
const PENDING_LISTINGS: usize = 256;
/// How many files are identified in one batch.
const IDENTIFY_BATCH: usize = 64;
/// How often progress is reported during a walk.
const PROGRESS_EVERY: Duration = Duration::from_millis(250);

/// What to scan.
#[derive(Clone, Copy, Debug)]
pub struct Job<'a> {
    /// The media folder it is in.
    pub root: &'a Root,
    /// The folder to walk from.
    pub dir: &'a Path,
    /// How deep, and whether hidden files count.
    pub options: walk::Options,
    /// Which files to identify again.
    pub rescan: Rescan,
}

/// What a scan found besides the files it wrote.
#[derive(Debug, Default)]
pub struct Outcome {
    /// Folders that couldn't be read for lack of permission.
    pub unreadable: Vec<PathBuf>,
    /// Subfolders a shallow scan found that the library didn't have.
    pub new_subdirs: Vec<PathBuf>,
}

/// Walks `job`, writing what it finds, and calls `progress` now and then.
pub fn scan(
    store: &dyn Store,
    job: Job<'_>,
    cancel: &Cancel,
    progress: &mut dyn FnMut(Progress),
) -> Outcome {
    let (case, dir) = (job.root.case, job.dir.to_path_buf());
    let before = call(store, move |c| match folders::find(c, &dir, case)? {
        Some(folder) => files::count_below(c, folder),
        None => Ok(0),
    })
    .unwrap_or(0);
    let counters = Counters::default();
    let (sink, events) = mpsc::sync_channel(PENDING_LISTINGS);
    let mut writer =
        Writer { store, job, outcome: Outcome::default(), batch: Vec::new(), found: 0 };
    thread::scope(|scope| {
        let counters = &counters;
        scope.spawn(move || walk::walk(job.dir, job.options, cancel, counters, &sink));
        let mut reported = Instant::now();
        for event in events {
            writer.take(event);
            if reported.elapsed() >= PROGRESS_EVERY {
                reported = Instant::now();
                progress(Progress::estimate(writer.sample(counters, before)));
            }
        }
    });
    writer.flush();
    writer.outcome
}

/// Writes listings as they come, in batches.
struct Writer<'a> {
    store: &'a dyn Store,
    job: Job<'a>,
    outcome: Outcome,
    batch: Vec<Listing>,
    found: usize,
}

impl Writer<'_> {
    fn take(&mut self, event: Walked) {
        match event {
            Walked::Listed(listing) => self.add(listing),
            Walked::Unreadable(dir, kind) => self.skip(dir, kind),
        }
    }

    /// Skips a folder the walk couldn't read; one the user may not read is reported back.
    fn skip(&mut self, dir: PathBuf, kind: std::io::ErrorKind) {
        tracing::debug!("library: skipped {}: {kind}", crate::logging::path(&dir));
        if kind == std::io::ErrorKind::PermissionDenied {
            self.outcome.unreadable.push(dir);
        }
    }

    /// Batches `listing`, writing the batch once it is big enough.
    fn add(&mut self, listing: Listing) {
        self.found += listing.files.len();
        self.batch.push(listing);
        let files: usize = self.batch.iter().map(|listing| listing.files.len()).sum();
        if files >= BATCH_FILES || self.batch.len() >= BATCH_LISTINGS {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if self.batch.is_empty() {
            return;
        }
        let listings = std::mem::take(&mut self.batch);
        let (case, rescan) = (self.job.root.case, self.job.rescan);
        let applied = call(self.store, move |c| apply::apply_all(c, &listings, case, rescan));
        if let Some(applied) = applied
            && self.job.options.depth == Depth::Shallow
        {
            self.outcome.new_subdirs.extend(applied.new_subdirs);
        }
    }

    fn sample(&self, counters: &Counters, before: usize) -> Sample {
        Sample {
            found: self.found,
            listed: counters.listed.load(Ordering::Relaxed),
            folders: counters.found.load(Ordering::Relaxed),
            before,
        }
    }
}

/// Identifies every file waiting to be: answers whether some were still changing, so the
/// indexer should come back to them.
pub fn identify(store: &dyn Store, cancel: &Cancel) -> bool {
    let mut after = FileId(0);
    let mut unsettled = false;
    while !cancel.is_cancelled() {
        let batch = call(store, move |c| files::unidentified(c, after, IDENTIFY_BATCH));
        let Some(last) = batch.as_ref().and_then(|batch| batch.last()) else { break };
        after = last.id;
        let read = identify::read(batch.as_deref().unwrap_or_default(), now_ms(), cancel);
        unsettled |= read.unsettled;
        let identified = read.identified;
        call(store, move |c| identify::link(c, &identified));
    }
    unsettled
}
