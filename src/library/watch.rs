//! Live updates (S4.2.4): a file system watcher on every media folder (FSEvents on macOS,
//! ReadDirectoryChangesW on Windows, inotify on Linux, through the `notify` crate), whose events
//! are gathered for [`DEBOUNCE`] after the last one so a copy of a thousand files is one rescan.
//!
//! Watchers drop events (overflowing buffers, network shares, sleep), so the indexer also
//! reconciles every media folder now and then; a watcher that can't start only costs speed.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};

/// How long after the last change the changes are rescanned.
pub const DEBOUNCE: Duration = Duration::from_secs(2);

/// Watches the media folders, calling back with the paths that changed.
pub struct Watcher {
    inner: Option<RecommendedWatcher>,
    watched: BTreeSet<PathBuf>,
}

impl Watcher {
    /// A watcher calling `changed` (on a thread of its own) with each event's paths.
    pub fn new(changed: impl Fn(Vec<PathBuf>) + Send + 'static) -> Self {
        let handler = move |event: notify::Result<notify::Event>| match event {
            Ok(event) if !matches!(event.kind, EventKind::Access(_)) => changed(event.paths),
            Ok(_) => {}
            Err(error) => tracing::debug!("library: watcher: {error}"),
        };
        let inner = notify::recommended_watcher(handler)
            .inspect_err(|error| tracing::warn!("library: no live updates: {error}"))
            .ok();
        Self { inner, watched: BTreeSet::new() }
    }

    /// Watches exactly `folders` (and everything below them).
    pub fn watch_only(&mut self, folders: &[PathBuf]) {
        let Some(inner) = &mut self.inner else { return };
        let wanted: BTreeSet<PathBuf> = folders.iter().cloned().collect();
        for gone in self.watched.difference(&wanted) {
            let _ = inner.unwatch(gone);
        }
        for new in wanted.difference(&self.watched) {
            if let Err(error) = inner.watch(new, RecursiveMode::Recursive) {
                tracing::warn!("library: can't watch {}: {error}", crate::logging::path(new));
            }
        }
        self.watched = wanted;
    }
}

/// Changed paths waiting for the changes to settle.
#[derive(Debug, Default)]
pub struct Debounce {
    paths: BTreeSet<PathBuf>,
    due: Option<Instant>,
}

impl Debounce {
    /// Adds `paths` that changed at `now`, which puts the rescan off until [`DEBOUNCE`] later.
    pub fn add(&mut self, paths: impl IntoIterator<Item = PathBuf>, now: Instant) {
        self.paths.extend(paths);
        if !self.paths.is_empty() {
            self.due = Some(now + DEBOUNCE);
        }
    }

    /// When the waiting paths are due, if any are waiting.
    pub fn due(&self) -> Option<Instant> {
        self.due
    }

    /// The waiting paths, if they are due at `now`, outermost first and without the ones inside
    /// another.
    pub fn take_due(&mut self, now: Instant) -> Vec<PathBuf> {
        if self.due.is_none_or(|due| now < due) {
            return Vec::new();
        }
        self.due = None;
        let mut outermost: Vec<PathBuf> = Vec::new();
        // In sorted order a folder comes right before what is inside it.
        for path in std::mem::take(&mut self.paths) {
            if !outermost.last().is_some_and(|outer| is_inside(&path, outer)) {
                outermost.push(path);
            }
        }
        outermost
    }
}

fn is_inside(path: &Path, outer: &Path) -> bool {
    path.starts_with(outer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_wait_until_they_settle() {
        let start = Instant::now();
        let mut debounce = Debounce::default();
        debounce.add([PathBuf::from("/v/a.mkv")], start);
        debounce.add([PathBuf::from("/v/b.mkv")], start + Duration::from_secs(1));
        assert_eq!(debounce.take_due(start + DEBOUNCE), Vec::<PathBuf>::new());
        let due = start + Duration::from_secs(1) + DEBOUNCE;
        assert_eq!(debounce.due(), Some(due));
        assert_eq!(debounce.take_due(due), [PathBuf::from("/v/a.mkv"), PathBuf::from("/v/b.mkv")]);
        assert_eq!(debounce.due(), None);
        assert_eq!(debounce.take_due(due), Vec::<PathBuf>::new());
    }

    #[test]
    fn paths_inside_another_changed_path_are_folded_into_it() {
        let now = Instant::now();
        let mut debounce = Debounce::default();
        let paths = ["/v/show/e1.mkv", "/v/show", "/v/showreel.mkv", "/v/show/extras/x.mkv"];
        debounce.add(paths.map(PathBuf::from), now);
        let due = debounce.take_due(now + DEBOUNCE);
        assert_eq!(due, [PathBuf::from("/v/show"), PathBuf::from("/v/showreel.mkv")]);
    }

    #[test]
    fn nothing_waiting_is_never_due() {
        let now = Instant::now();
        let mut debounce = Debounce::default();
        debounce.add([], now);
        assert_eq!(debounce.due(), None);
    }

    #[test]
    fn a_watcher_starts_and_follows_its_folders() {
        let dir = crate::opening::scratch::scratch_dir();
        let mut watcher = Watcher::new(|_| {});
        watcher.watch_only(std::slice::from_ref(&dir));
        assert_eq!(watcher.watched.len(), 1);
        watcher.watch_only(&[]);
        assert_eq!(watcher.watched, BTreeSet::new());
    }
}
