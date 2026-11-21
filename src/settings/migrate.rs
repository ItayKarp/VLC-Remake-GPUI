//! Schema migrations (S0.1.7).
//!
//! `schema_version` lives in `settings.toml`; it's mirrored into `local.toml` for
//! reference. Migration `N` in [`MIGRATIONS`] upgrades version `N` to `N + 1` and may
//! move data between the two files. Migrations work on documents, not typed structs,
//! so unknown keys and comments survive them.

use toml_edit::{DocumentMut, Item};

use super::document::{schema_version, set_schema_version};

/// The schema version this build writes.
pub const CURRENT_VERSION: i64 = 1;

/// The two settings documents, migrated together because migrations may move data
/// between them.
pub struct Documents {
    /// `settings.toml`.
    pub prefs: DocumentMut,
    /// `local.toml`.
    pub local: DocumentMut,
}

/// Upgrades the documents by one schema version.
type Migration = fn(&mut Documents);

/// Every migration, in order: index `N` upgrades version `N`.
const MIGRATIONS: &[Migration] = &[migrate_v0_to_v1];

const _: () = assert!(MIGRATIONS.len() as i64 == CURRENT_VERSION);

/// What [`migrate`] did.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing to do: a fresh install or the current version.
    UpToDate,
    /// Upgraded from version `from`; the files need saving.
    Migrated { from: i64 },
    /// Written by a newer build. Loaded as-is, without downgrading the version.
    Newer { version: i64 },
}

/// Upgrades both documents to [`CURRENT_VERSION`].
pub fn migrate(docs: &mut Documents) -> Outcome {
    if docs.prefs.is_empty() && docs.local.is_empty() {
        // Fresh install: nothing to migrate, and nothing worth writing yet.
        set_schema_version(&mut docs.prefs, CURRENT_VERSION);
        set_schema_version(&mut docs.local, CURRENT_VERSION);
        return Outcome::UpToDate;
    }
    let version = schema_version(&docs.prefs);
    if version > CURRENT_VERSION {
        return newer(version);
    }
    // A negative version can only come from a hand edit; treat it as the oldest.
    let from = version.max(0);
    for migration in &MIGRATIONS[from as usize..] {
        migration(docs);
    }
    set_schema_version(&mut docs.prefs, CURRENT_VERSION);
    set_schema_version(&mut docs.local, CURRENT_VERSION);
    if from == CURRENT_VERSION { Outcome::UpToDate } else { migrated(from) }
}

// The logging lives in helpers: with `tracing`'s `log` feature (on in Linux builds) two
// macros push `migrate` past the complexity limit.

/// Reports files written by a newer build, which are loaded without downgrading.
fn newer(version: i64) -> Outcome {
    tracing::warn!(
        "settings: schema version {version} is newer than this build ({CURRENT_VERSION}); \
         unknown settings will be kept but ignored"
    );
    Outcome::Newer { version }
}

/// Reports a finished migration from version `from`.
fn migrated(from: i64) -> Outcome {
    tracing::info!("settings: migrated from schema version {from} to {CURRENT_VERSION}");
    Outcome::Migrated { from }
}

/// The first onboarding build wrote everything into `settings.toml`:
///
/// ```toml
/// onboarding_complete = true
/// media_folder = "C:\\Users\\me\\Videos"
/// theme = "dark"
/// accent = "orange"
/// ```
///
/// v1 moves the machine-specific keys to `local.toml` and makes the folder a list.
fn migrate_v0_to_v1(docs: &mut Documents) {
    if let Some(item) = docs.prefs.remove("onboarding_complete")
        && !docs.local.contains_key("onboarding_complete")
    {
        docs.local["onboarding_complete"] = without_decor(item);
    }
    if let Some(item) = docs.prefs.remove("media_folder")
        && !docs.local.contains_key("media_folders")
    {
        let mut folders = toml_edit::Array::new();
        if let Some(folder) = item.as_str().filter(|folder| !folder.is_empty()) {
            folders.push(folder);
        }
        docs.local["media_folders"] = toml_edit::value(folders);
    }
}

fn without_decor(mut item: Item) -> Item {
    if let Some(value) = item.as_value_mut() {
        value.decor_mut().clear();
    }
    item
}
