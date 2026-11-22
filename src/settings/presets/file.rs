//! Preset files (S5.11.4), for sharing an "Anime" preset: TOML with the preset's name and the
//! values it changes, nested as in the settings file.
//!
//! ```toml
//! vlc_preset = 1
//! name = "Anime"
//!
//! [values.subtitles]
//! font = "Noto Sans"
//! ```
//!
//! Reading keeps only the Playback, Video, Audio and Subtitles values, so a shared file can't
//! change anything else (the proxy, the log file), and keeps values this build doesn't know.

use std::path::Path;

use super::overlay;
use crate::settings::Preset;

/// The extension preset files are saved with.
pub const EXTENSION: &str = "vlcpreset";
/// The key that marks a file as a preset, and its format's version.
const MARKER: &str = "vlc_preset";
/// The format written.
const VERSION: i64 = 1;

/// The file's text for `preset`.
pub fn render(preset: &Preset) -> String {
    let mut table = toml::Table::new();
    table.insert(MARKER.into(), VERSION.into());
    table.insert("name".into(), preset.name.clone().into());
    table.insert("values".into(), preset.values.clone().into());
    let body = toml::to_string(&table).unwrap_or_else(|err| {
        tracing::error!("settings: couldn't write preset “{}”: {err}", preset.name);
        String::new()
    });
    format!("# A VLC settings preset. Import it from the preset list in Settings.\n{body}")
}

/// The preset in a file's `text`; `path` names it when the file has no name. Fails with what to
/// tell the user.
pub fn parse(text: &str, path: &Path) -> Result<Preset, String> {
    let not_a_preset = || "This file isn't a VLC settings preset.".to_owned();
    let table: toml::Table = toml::from_str(text).map_err(|_| not_a_preset())?;
    if !table.contains_key(MARKER) {
        return Err(not_a_preset());
    }
    let named = table.get("name").and_then(toml::Value::as_str).map(str::to_owned);
    let name = named.unwrap_or_else(|| {
        path.file_stem().map_or_default(|stem| stem.to_string_lossy().into_owned())
    });
    let mut values =
        table.get("values").and_then(toml::Value::as_table).cloned().unwrap_or_default();
    values.retain(|section, _| overlay::SECTIONS.contains(&section));
    Ok(Preset { name, values })
}

/// A file name for `name`: characters file systems refuse become dashes.
pub fn file_name(name: &str) -> String {
    const UNSAFE: [char; 9] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
    format!("{}.{EXTENSION}", name.replace(UNSAFE, "-"))
}
