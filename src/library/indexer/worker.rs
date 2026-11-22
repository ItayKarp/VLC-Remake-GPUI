//! The indexer thread: keeps the media folders in the database, scans them, follows the
//! watcher, and reconciles every [`RECONCILE_EVERY`] because watchers drop events (S4.2.4).
//! A media folder that is missing is checked again every [`RECHECK_EVERY`], so a drive that comes
//! back is indexed soon after; its files stay in the library meanwhile, with their history.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use super::store::{Store, call};
use super::targets::{self, Target, Watched};
use super::{Command, RootState, Wanted, scan};
use crate::db::files;
use crate::db::now_ms;
use crate::db::roots::{self, Root, RootStatus};
use crate::library::apply::Rescan;
use crate::library::extract::{Extractor, Pass};
use crate::library::identify::STABLE_AFTER;
use crate::library::path_key::Case;
use crate::library::root_check;
use crate::library::walk::{self, Depth};
use crate::library::watch::{Debounce, Watcher};
use crate::opening::Cancel;

/// How often every media folder is rescanned, for the changes the watcher missed.
pub const RECONCILE_EVERY: Duration = Duration::from_mins(15);
/// How often a missing or unanswering media folder is checked again.
pub const RECHECK_EVERY: Duration = Duration::from_secs(30);

/// Where the indexer's news goes: every media folder's state, each time one changes.
pub type Report = Box<dyn Fn(Vec<RootState>) + Send>;

/// Starts the indexer thread on `store`, reading the metadata of what it finds with `extractor`
/// (S4.3). It stops once `cancel` is set.
pub fn spawn(
    store: Box<dyn Store>,
    extractor: Extractor,
    report: Report,
    cancel: Cancel,
) -> Sender<Command> {
    let (sender, commands) = mpsc::channel();
    let changes = sender.clone();
    let started = std::thread::Builder::new().name("library".into()).spawn(move || {
        let watcher = Watcher::new(move |paths| {
            let _ = changes.send(Command::Changed(paths));
        });
        let now = Instant::now();
        let timers = Timers {
            reconcile: now + RECONCILE_EVERY,
            recheck: None,
            identify: None,
            extract: None,
        };
        let worker = Worker {
            store,
            extractor,
            report,
            cancel,
            roots: Vec::new(),
            states: Vec::new(),
            unavailable: Vec::new(),
            show_hidden: false,
            watcher,
            debounce: Debounce::default(),
            timers,
        };
        worker.run(&commands);
    });
    if let Err(error) = started {
        tracing::error!("library: couldn't start the indexer: {error}");
    }
    sender
}

struct Worker {
    store: Box<dyn Store>,
    extractor: Extractor,
    report: Report,
    cancel: Cancel,
    roots: Vec<Watched>,
    states: Vec<RootState>,
    /// Per media folder: whether its last check couldn't scan it, so it is checked again soon.
    unavailable: Vec<bool>,
    show_hidden: bool,
    watcher: Watcher,
    debounce: Debounce,
    timers: Timers,
}

/// When the indexer next has something to do by itself.
struct Timers {
    reconcile: Instant,
    recheck: Option<Instant>,
    identify: Option<Instant>,
    /// Metadata extraction has media waiting (S4.3): it runs a batch at a time, between commands.
    extract: Option<Instant>,
}

impl Worker {
    fn run(mut self, commands: &Receiver<Command>) {
        while !self.cancel.is_cancelled() {
            let timers = &self.timers;
            let next = [Some(timers.reconcile), timers.recheck, timers.identify, timers.extract];
            let next = next.into_iter().flatten().chain(self.debounce.due()).min();
            let wait =
                next.map_or(RECONCILE_EVERY, |at| at.saturating_duration_since(Instant::now()));
            match commands.recv_timeout(wait) {
                Ok(command) => self.handle(command),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            self.run_due(Instant::now());
        }
    }

    fn handle(&mut self, command: Command) {
        match command {
            Command::SetRoots(wanted) => {
                self.set_roots(wanted);
                self.refresh_all(Rescan::Changed);
            }
            Command::ShowHidden(show) if show != self.show_hidden => {
                self.show_hidden = show;
                self.refresh_all(Rescan::Changed);
            }
            Command::ShowHidden(_) => {}
            Command::Lookup(lookup) => self.extractor.set_lookup(lookup),
            Command::Resync(path) => self.resync(&path),
            Command::Changed(paths) => self.debounce.add(paths, Instant::now()),
        }
    }

    fn run_due(&mut self, now: Instant) {
        let changed = self.debounce.take_due(now);
        if !changed.is_empty() {
            self.rescan_changed(&changed);
        }
        if now >= self.timers.reconcile {
            self.timers.reconcile = now + RECONCILE_EVERY;
            self.refresh_all(Rescan::Changed);
        }
        if self.timers.recheck.is_some_and(|at| now >= at) {
            self.timers.recheck = None;
            self.recheck_unavailable();
        }
        if self.timers.identify.is_some_and(|at| now >= at) {
            self.identify();
            self.report();
        }
        if self.timers.extract.is_some_and(|at| now >= at) {
            let pass = self.extractor.run(&*self.store, &self.cancel);
            self.timers.extract = (pass == Pass::More).then(Instant::now);
        }
    }

    /// Makes the database's media folders exactly `wanted`.
    fn set_roots(&mut self, wanted: Vec<Wanted>) {
        let cases: Vec<Case> = wanted.iter().map(|w| Case::detect(&w.path)).collect();
        let roots = call(&*self.store, move |c| sync_roots(c, &wanted, &cases));
        self.roots = roots.unwrap_or_default().into_iter().map(Watched::new).collect();
        self.states = self.roots.iter().map(|w| RootState::of(&w.root)).collect();
        // Unchecked folders aren't watched until their check says they can be read.
        self.unavailable = vec![true; self.roots.len()];
    }

    /// Checks and rescans every media folder, then identifies what changed.
    fn refresh_all(&mut self, rescan: Rescan) {
        for index in 0..self.roots.len() {
            self.refresh_root(index, rescan);
        }
        self.identify();
        self.report();
    }

    /// Checks media folder `index` and, if it can be read, rescans it whole.
    fn refresh_root(&mut self, index: usize, rescan: Rescan) {
        let root = self.roots[index].root.clone();
        let checked = root_check::check(&root.path);
        self.set_status(index, checked.status);
        self.unavailable[index] = !checked.scannable;
        if !checked.scannable {
            self.timers.recheck.get_or_insert(Instant::now() + RECHECK_EVERY);
            self.update_watches();
            return;
        }
        self.update_watches();
        let unreadable = self.scan(index, &root.path, Depth::Tree, rescan);
        let id = root.id;
        call(&*self.store, move |c| {
            roots::set_unreadable(c, id, &unreadable)?;
            roots::set_scanned(c, id, now_ms())
        });
    }

    /// Walks `dir` of media folder `index`, and the new subfolders a shallow walk finds. Answers
    /// the folders that couldn't be read.
    fn scan(&mut self, index: usize, dir: &Path, depth: Depth, rescan: Rescan) -> Vec<PathBuf> {
        let root = self.roots[index].root.clone();
        let options = walk::Options { depth, show_hidden: self.show_hidden };
        let job = scan::Job { root: &root, dir, options, rescan };
        let (store, report, states) = (&*self.store, &self.report, &mut self.states);
        let outcome = scan::scan(store, job, &self.cancel, &mut |progress| {
            states[index].indexing = Some(progress);
            report(states.clone());
        });
        self.states[index].indexing = None;
        let mut unreadable = outcome.unreadable;
        for new in outcome.new_subdirs {
            unreadable.extend(self.scan(index, &new, Depth::Tree, rescan));
        }
        unreadable
    }

    fn rescan_changed(&mut self, paths: &[PathBuf]) {
        let mut targets: Vec<Target> = Vec::new();
        for path in paths {
            let Some(target) = targets::target(&*self.store, &self.roots, path) else { continue };
            if !targets.contains(&target) {
                targets.push(target);
            }
        }
        for target in targets {
            self.rescan(&target, Rescan::Changed);
        }
        self.identify();
        self.report();
    }

    /// Resync (S4.2.5): rescans the folder at `path` and everything below it, reading every file
    /// again.
    fn resync(&mut self, path: &Path) {
        let index = self.roots.iter().position(|w| path.starts_with(&w.root.path));
        let Some(index) = index else { return };
        let target = Target { root: index, dir: path.to_path_buf(), depth: Depth::Tree };
        self.rescan(&target, Rescan::Everything);
        self.identify();
        self.report();
    }

    fn rescan(&mut self, target: &Target, rescan: Rescan) {
        let root = &self.roots[target.root].root;
        if target.dir == root.path && target.depth == Depth::Tree {
            self.refresh_root(target.root, rescan);
        } else if !self.unavailable[target.root] {
            self.scan(target.root, &target.dir, target.depth, rescan);
        }
    }

    /// Checks the media folders that were missing, unreadable or didn't answer.
    fn recheck_unavailable(&mut self) {
        let waiting: Vec<usize> =
            (0..self.roots.len()).filter(|&index| self.unavailable[index]).collect();
        for index in waiting {
            self.refresh_root(index, Rescan::Changed);
        }
        self.identify();
        self.report();
    }

    fn identify(&mut self) {
        let unsettled = scan::identify(&*self.store, &self.cancel);
        let now = Instant::now();
        self.timers.identify = unsettled.then(|| now + STABLE_AFTER);
        self.timers.extract = Some(now);
    }

    fn set_status(&mut self, index: usize, status: RootStatus) {
        if self.states[index].status == status && self.roots[index].root.status == status {
            return;
        }
        self.states[index].status = status;
        self.roots[index].root.status = status;
        let id = self.roots[index].root.id;
        call(&*self.store, move |c| roots::set_status(c, id, status));
        self.report();
    }

    fn update_watches(&mut self) {
        let readable: Vec<PathBuf> = (0..self.roots.len())
            .filter(|&index| !self.unavailable[index])
            .map(|index| self.roots[index].root.path.clone())
            .collect();
        self.watcher.watch_only(&readable);
    }

    /// Sends every media folder's state, with fresh counts.
    fn report(&mut self) {
        let roots: Vec<Root> = self.roots.iter().map(|w| w.root.clone()).collect();
        let counts = call(&*self.store, move |c| {
            let count = |root: &Root| -> rusqlite::Result<(usize, usize)> {
                Ok((files::count_below(c, root.folder)?, roots::unreadable(c, root.id)?.len()))
            };
            roots.iter().map(count).collect::<rusqlite::Result<Vec<_>>>()
        });
        for (state, (files, unreadable)) in self.states.iter_mut().zip(counts.unwrap_or_default()) {
            state.files = files;
            state.unreadable = unreadable;
        }
        (self.report)(self.states.clone());
    }
}

/// Removes the media folders not in `wanted` and adds the new ones (each with its `cases` entry),
/// then lists them.
fn sync_roots(
    connection: &mut rusqlite::Connection,
    wanted: &[Wanted],
    cases: &[Case],
) -> rusqlite::Result<Vec<Root>> {
    let unwanted = roots::list(connection)?
        .into_iter()
        .filter(|root| !wanted.iter().any(|w| w.path == root.path));
    for root in unwanted {
        roots::remove(connection, root.id)?;
    }
    for (wanted, &case) in wanted.iter().zip(cases) {
        roots::add(connection, &wanted.path, wanted.kind, case, now_ms())?;
    }
    roots::list(connection)
}
