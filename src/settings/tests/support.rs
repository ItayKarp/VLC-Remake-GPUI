//! Shared helpers for the settings tests: a throwaway config directory and a
//! synchronous save.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use crate::settings::paths::Locations;
use crate::settings::store::SettingsState;

/// A fresh directory per test, removed on drop.
pub(super) struct TempDir(PathBuf);

impl TempDir {
    /// Creates an empty directory that no other test uses.
    pub(super) fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "rvlc-settings-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    /// The directory itself.
    pub(super) fn root(&self) -> &Path {
        &self.0
    }

    /// Settings locations with both files in this directory.
    pub(super) fn locations(&self) -> Locations {
        Locations::in_dir(&self.0)
    }

    /// The path of `name` inside this directory.
    pub(super) fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    /// Writes `contents` to `name` inside this directory.
    pub(super) fn write(&self, name: &str, contents: &str) {
        fs::write(self.path(name), contents).unwrap();
    }

    /// Reads `name` inside this directory.
    pub(super) fn read(&self, name: &str) -> String {
        fs::read_to_string(self.path(name)).unwrap()
    }

    /// Loads settings from this directory.
    pub(super) fn load(&self) -> SettingsState {
        SettingsState::load(Some(self.locations()))
    }

    /// The sorted names of the files in this directory.
    pub(super) fn names(&self) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(&self.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Runs a save to completion on this thread, asserting it succeeds.
pub(super) fn save(state: &mut SettingsState) {
    let job = state.prepare_save().expect("something to save");
    let generation = job.generation();
    let result = job.run();
    assert_eq!(result, Ok(()));
    state.finish_save(generation, result);
}

/// Paths that are hard to store: quotes, separators, control characters and
/// non-ASCII text.
pub(super) fn awkward_paths() -> Vec<PathBuf> {
    vec![
        PathBuf::from(r#"C:\Users\me\My "quoted" Videos"#),
        PathBuf::from(r"\\server\share\trailing\"),
        PathBuf::from("/home/me/line\nbreak/and\ttab"),
        PathBuf::from("/home/me/Видео/🎬 films/שלום"),
        PathBuf::from("/home/me/os-path:looks-encoded"),
        PathBuf::from("/home/me/'single' and \"double\" and \\back"),
    ]
}
