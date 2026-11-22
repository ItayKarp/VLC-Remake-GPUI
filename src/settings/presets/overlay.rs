//! Presets as values of the settings file: the Playback, Video, Audio and Subtitles sections
//! flattened into [`Leaves`] by path (`audio.max_volume_percent`), and leaves written back over
//! the typed preferences.
//!
//! Working on the file's values rather than on the registry's rows means everything those pages
//! save is covered, the equalizer's bands included, and a value this build doesn't know passes
//! through untouched.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::settings::Preferences;

/// The sections of [`Preferences`] a preset covers: `player` (Playback), `video`, `audio` and
/// `subtitles`. [`apply`] copies exactly these fields back.
pub const SECTIONS: [&str; 4] = ["player", "video", "audio", "subtitles"];

/// Values by their dotted path in the settings file; tables are walked into, everything else
/// (arrays included) is one value.
pub type Leaves = BTreeMap<String, toml::Value>;

/// The values of `prefs`' preset sections.
pub fn leaves(prefs: &Preferences) -> Leaves {
    let mut leaves = Leaves::new();
    let Ok(toml::Value::Table(table)) = toml::Value::try_from(prefs) else {
        tracing::error!("settings: couldn't read the preferences as values");
        return leaves;
    };
    for section in SECTIONS {
        if let Some(value) = table.get(section) {
            flatten_into(section.to_owned(), value, &mut leaves);
        }
    }
    leaves
}

/// Whether the preset sections of `a` and `b` hold the same values.
pub fn same_sections(a: &Preferences, b: &Preferences) -> bool {
    a.player == b.player && a.video == b.video && a.audio == b.audio && a.subtitles == b.subtitles
}

/// A nested table's values by path.
pub fn flatten(table: &toml::Table) -> Leaves {
    let mut leaves = Leaves::new();
    for (key, value) in table {
        flatten_into(key.clone(), value, &mut leaves);
    }
    leaves
}

/// `leaves` as a nested table, the way the settings file writes them.
pub fn nest(leaves: &Leaves) -> toml::Table {
    let mut root = toml::Table::new();
    for (path, value) in leaves {
        let (parents, key) = path.rsplit_once('.').unwrap_or(("", path.as_str()));
        let mut parents = parents.split('.').filter(|part| !part.is_empty());
        if let Some(table) = parents.try_fold(&mut root, |table, part| {
            let entry = table.entry(part).or_insert_with(|| toml::Table::new().into());
            entry.as_table_mut()
        }) {
            table.insert(key.to_owned(), value.clone());
        }
    }
    root
}

/// Writes `leaves` over `prefs`' preset sections. A path the sections don't have, or a value of
/// another kind than the setting's, is skipped, so a preset from another version never breaks a
/// setting it doesn't fit.
pub fn apply(prefs: &mut Preferences, leaves: &Leaves) {
    let Ok(toml::Value::Table(mut table)) = toml::Value::try_from(&*prefs) else { return };
    let mut changed = false;
    for (path, value) in leaves.iter().filter(|(path, _)| in_sections(path)) {
        if let Some(slot) = slot_mut(&mut table, path)
            && same_kind(slot, value)
            && slot != value
        {
            *slot = value.clone();
            changed = true;
        }
    }
    if !changed {
        return;
    }
    match Preferences::deserialize(toml::Value::Table(table)) {
        Ok(applied) => {
            prefs.player = applied.player;
            prefs.video = applied.video;
            prefs.audio = applied.audio;
            prefs.subtitles = applied.subtitles;
        }
        Err(err) => tracing::warn!("settings: couldn't apply a preset: {err}"),
    }
}

/// Whether `path` is inside one of the preset sections.
pub fn in_sections(path: &str) -> bool {
    let section = path.split('.').next().unwrap_or_default();
    SECTIONS.contains(&section)
}

fn flatten_into(path: String, value: &toml::Value, leaves: &mut Leaves) {
    match value {
        toml::Value::Table(table) => {
            for (key, inner) in table {
                flatten_into(format!("{path}.{key}"), inner, leaves);
            }
        }
        other => {
            leaves.insert(path, other.clone());
        }
    }
}

/// The value at `path` in `table`, unless it's a table itself.
fn slot_mut<'a>(table: &'a mut toml::Table, path: &str) -> Option<&'a mut toml::Value> {
    let mut parts = path.split('.');
    let mut value = table.get_mut(parts.next()?)?;
    for part in parts {
        value = value.as_table_mut()?.get_mut(part)?;
    }
    if value.is_table() { None } else { Some(value) }
}

/// Whether `new` can stand where `old` is: the same kind of value, a number for a number.
fn same_kind(old: &toml::Value, new: &toml::Value) -> bool {
    use toml::Value::{Float, Integer};
    let numbers = matches!((old, new), (Float(_) | Integer(_), Float(_) | Integer(_)));
    numbers || old.type_str() == new.type_str()
}
