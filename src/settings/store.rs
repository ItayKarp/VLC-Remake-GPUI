//! Settings state without GPUI: loading, change tracking and preparing saves. The
//! GPUI global in `settings.rs` wraps this so it can be unit-tested directly.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use toml_edit::DocumentMut;

use super::document::merge_typed;
use super::migrate::{self, Documents, Outcome};
use super::paths::Locations;
use super::presets::Layer;
use super::registry;
use super::schema::{LiveState, LocalState, Preferences, Settings};
use super::storage::{self, LoadIssue};

/// Something the UI should tell the user once (S0.2/S0.3 show these).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    /// A settings file didn't parse; it was kept as `moved_to`.
    Corrupt { file: PathBuf, moved_to: Option<PathBuf>, restored_from_backup: bool },
    /// A settings file was empty and its backup was used.
    RestoredFromBackup { file: PathBuf },
    /// Changes won't be saved this session (no config directory, or a file that can't
    /// be read or safely replaced). Show a banner.
    NotPersisted { reason: String },
    /// A save failed (read-only directory, disk full, …). Reported once per session;
    /// the next change retries.
    SaveFailed { error: String },
}

/// One settings file on disk and its editable, comment-preserving document.
struct FileSlot {
    path: PathBuf,
    doc: DocumentMut,
    /// False when overwriting the file could destroy data we couldn't read.
    writable: bool,
}

impl FileSlot {
    /// Reads the file at `path`, recording anything the user should hear about in
    /// `notices`.
    fn load(path: PathBuf, notices: &mut Vec<Notice>) -> Self {
        match storage::load(&path) {
            Ok(loaded) => {
                let writable = record_issue(&path, loaded.issue, notices);
                FileSlot { path, doc: loaded.doc, writable }
            }
            Err(err) => {
                tracing::warn!("settings: couldn't read {}: {err}", path.display());
                notices.push(Notice::NotPersisted {
                    reason: tr!("Couldn't read {path}: {err}", path = path.display(), err),
                });
                FileSlot { path, doc: DocumentMut::new(), writable: false }
            }
        }
    }

    /// Merges `typed` into the document and queues the result for writing, unless the
    /// file must not be overwritten.
    fn render<T: serde::Serialize>(&mut self, typed: &T, writes: &mut Vec<(PathBuf, String)>) {
        match merge_typed(&mut self.doc, typed) {
            Ok(()) if self.writable => writes.push((self.path.clone(), self.doc.to_string())),
            Ok(()) => {}
            Err(err) => {
                tracing::error!("settings: couldn't serialize {}: {err}", self.path.display());
            }
        }
    }
}

/// The loaded settings, the documents they came from, and the bookkeeping that decides
/// when and what to save.
pub struct SettingsState {
    /// The current typed settings; change them through [`Self::update`].
    pub settings: Settings,
    /// The defaults the preset in use covers (S5.11).
    presets: Layer,
    /// `None`: no config directory, settings live in memory only.
    files: Option<[FileSlot; 2]>,
    dirty: bool,
    /// Incremented by each prepared save.
    generation: u64,
    /// Newest generation known to be on disk.
    confirmed: u64,
    /// Generation of the newest save written, so an older save that finishes late
    /// never overwrites a newer one.
    written: Arc<Mutex<u64>>,
    save_failure_reported: bool,
    notices: Vec<Notice>,
}

/// A prepared save that can run on any thread.
pub struct SaveJob {
    writes: Vec<(PathBuf, String)>,
    generation: u64,
    written: Arc<Mutex<u64>>,
}

impl SaveJob {
    /// The save's generation, to pass back to [`SettingsState::finish_save`].
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Writes the files, unless a newer save has already been written.
    pub fn run(self) -> Result<(), String> {
        let mut written = self.written.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if *written > self.generation {
            return Ok(());
        }
        for (path, contents) in &self.writes {
            storage::save(path, contents)
                .map_err(|err| tr!("couldn't save {path}: {err}", path = path.display(), err))?;
        }
        *written = self.generation;
        Ok(())
    }
}

impl SettingsState {
    /// Loads and migrates both settings files, or starts from defaults in memory when
    /// there is no config directory.
    pub fn load(locations: Option<Locations>) -> Self {
        let mut notices = Vec::new();
        let Some(locations) = locations else {
            tracing::warn!("settings: no config directory; changes won't be saved");
            notices.push(Notice::NotPersisted {
                reason: tr!("No settings folder is available on this system.").into(),
            });
            return Self::new(Settings::default(), None, false, notices);
        };

        let mut prefs = FileSlot::load(locations.prefs, &mut notices);
        let mut local = FileSlot::load(locations.local, &mut notices);

        let mut docs = Documents {
            prefs: std::mem::take(&mut prefs.doc),
            local: std::mem::take(&mut local.doc),
        };
        let migrated = matches!(migrate::migrate(&mut docs), Outcome::Migrated { .. });
        prefs.doc = docs.prefs;
        local.doc = docs.local;

        let mut settings = Settings {
            prefs: deserialize::<Preferences>(&prefs.doc),
            local: deserialize::<LocalState>(&local.doc),
            live: LiveState::default(),
        };
        // Values from an older or newer version may be outside today's ranges.
        let clamped = registry::sanitize(&mut settings);
        let mut state = Self::new(settings, Some([prefs, local]), migrated || clamped, notices);
        state.presets = Layer::load(&mut state.settings.prefs);
        // A preset's values may be outside today's ranges too; they're clamped where they
        // apply and kept as they are in the preset.
        registry::sanitize(&mut state.settings);
        state
    }

    /// Assembles a state with no saves prepared yet.
    fn new(
        settings: Settings,
        files: Option<[FileSlot; 2]>,
        dirty: bool,
        notices: Vec<Notice>,
    ) -> Self {
        SettingsState {
            settings,
            presets: Layer::default(),
            files,
            dirty,
            generation: 0,
            confirmed: 0,
            written: Arc::default(),
            save_failure_reported: false,
            notices,
        }
    }

    /// Applies `f`; returns whether anything changed.
    pub fn update(&mut self, f: impl FnOnce(&mut Settings)) -> bool {
        let before = self.settings.clone();
        f(&mut self.settings);
        self.presets.record(&before.prefs, &mut self.settings.prefs);
        let changed = self.settings != before;
        // The live state is never written, so changing only it leaves nothing to save.
        self.dirty |= self.settings.prefs != before.prefs || self.settings.local != before.local;
        changed
    }

    /// Runs `f` on the presets and the preferences they cover; answers its result and whether
    /// anything changed.
    pub fn edit_presets<R>(
        &mut self,
        f: impl FnOnce(&mut Layer, &mut Preferences) -> R,
    ) -> (R, bool) {
        let before = self.settings.prefs.clone();
        let result = f(&mut self.presets, &mut self.settings.prefs);
        let changed = self.settings.prefs != before;
        self.dirty |= changed;
        (result, changed)
    }

    /// Whether there are changes that haven't been handed to a save yet.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Whether changes reach the disk: there is a config directory and every file may
    /// be overwritten.
    pub fn is_persistent(&self) -> bool {
        self.files.as_ref().is_some_and(|files| files.iter().all(|file| file.writable))
    }

    /// Returns the notices collected so far and forgets them.
    pub fn take_notices(&mut self) -> Vec<Notice> {
        std::mem::take(&mut self.notices)
    }

    /// Renders the files to write, or `None` if everything is saved. A save that was
    /// prepared but never confirmed (its task was cancelled) counts as unsaved.
    pub fn prepare_save(&mut self) -> Option<SaveJob> {
        if !self.dirty && self.confirmed == self.generation {
            return None;
        }
        self.dirty = false;
        let on_disk = self.presets.for_disk(&self.settings.prefs);
        let files = self.files.as_mut()?;
        let [prefs, local] = files;
        let mut writes = Vec::new();
        prefs.render(&*on_disk, &mut writes);
        local.render(&self.settings.local, &mut writes);
        self.generation += 1;
        Some(SaveJob { writes, generation: self.generation, written: self.written.clone() })
    }

    /// Records how a save ended. A failure keeps the changes dirty so the next change
    /// retries, and is reported once per session.
    pub fn finish_save(&mut self, generation: u64, result: Result<(), String>) {
        if result.is_ok() {
            self.confirmed = self.confirmed.max(generation);
        }
        if let Err(error) = result {
            tracing::warn!("settings: {error}");
            // Keep the changes in memory and retry on the next change.
            self.dirty = true;
            if !self.save_failure_reported {
                self.save_failure_reported = true;
                self.notices.push(Notice::SaveFailed { error });
            }
        }
    }
}

/// Turns a load issue into a notice and returns whether the file may be overwritten.
fn record_issue(path: &Path, issue: Option<LoadIssue>, notices: &mut Vec<Notice>) -> bool {
    match issue {
        Some(LoadIssue::RestoredFromBackup) => {
            notices.push(Notice::RestoredFromBackup { file: path.to_owned() });
            true
        }
        Some(LoadIssue::Corrupt { moved_to, restored_from_backup, .. }) => {
            // If the corrupt file couldn't be moved aside, saving over it would lose it.
            let writable = moved_to.is_some();
            notices.push(Notice::Corrupt { file: path.to_owned(), moved_to, restored_from_backup });
            writable
        }
        None => true,
    }
}

/// Reads a typed value out of a document, falling back to the default when the
/// document as a whole doesn't fit the schema.
fn deserialize<T: serde::de::DeserializeOwned + Default>(doc: &DocumentMut) -> T {
    toml_edit::de::from_document(doc.clone()).unwrap_or_else(|err| {
        tracing::warn!("settings: couldn't read settings, using defaults: {err}");
        T::default()
    })
}
