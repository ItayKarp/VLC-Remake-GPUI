//! Schema migrations (S0.1.7).
//!
//! `schema_version` lives in `settings.toml`; it's mirrored into `local.toml` for
//! reference. Migration `N` in [`MIGRATIONS`] upgrades version `N` to `N + 1` and may
//! move data between the two files. Migrations work on documents, not typed structs,
//! so unknown keys and comments survive them.

use toml_edit::{DocumentMut, Item};

use super::document::{schema_version, set_schema_version};

pub const CURRENT_VERSION: i64 = 1;

pub struct Documents {
    pub prefs: DocumentMut,
    pub local: DocumentMut,
}

type Migration = fn(&mut Documents);

const MIGRATIONS: &[Migration] = &[migrate_v0_to_v1];

const _: () = assert!(MIGRATIONS.len() as i64 == CURRENT_VERSION);

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    UpToDate,
    Migrated {
        from: i64,
    },
    /// Written by a newer build. Loaded as-is, without downgrading the version.
    Newer {
        version: i64,
    },
}

pub fn migrate(docs: &mut Documents) -> Outcome {
    if docs.prefs.is_empty() && docs.local.is_empty() {
        // Fresh install: nothing to migrate, and nothing worth writing yet.
        set_schema_version(&mut docs.prefs, CURRENT_VERSION);
        set_schema_version(&mut docs.local, CURRENT_VERSION);
        return Outcome::UpToDate;
    }
    let version = schema_version(&docs.prefs);
    if version > CURRENT_VERSION {
        log::warn!(
            "settings: schema version {version} is newer than this build ({CURRENT_VERSION}); \
             unknown settings will be kept but ignored"
        );
        return Outcome::Newer { version };
    }
    // A negative version can only come from a hand edit; treat it as the oldest.
    let from = version.max(0);
    for migration in &MIGRATIONS[from as usize..] {
        migration(docs);
    }
    set_schema_version(&mut docs.prefs, CURRENT_VERSION);
    set_schema_version(&mut docs.local, CURRENT_VERSION);
    if from == CURRENT_VERSION {
        Outcome::UpToDate
    } else {
        log::info!("settings: migrated from schema version {from} to {CURRENT_VERSION}");
        Outcome::Migrated { from }
    }
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
