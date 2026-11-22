//! Setting presets (S5.11): named overlays over the Playback, Video, Audio and Subtitles pages,
//! and which one is in use. `settings::presets` applies them.
//!
//! The presets are a list of tables rather than a table keyed by name, because saving merges
//! tables key by key: a deleted preset, or a value a preset no longer changes, would survive in
//! the file. A list is replaced whole.

use serde::{Deserialize, Serialize};

use super::super::lenient::lenient;

/// The presets the user made, and the one in use.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PresetSettings {
    /// The name of the preset in use; empty for Default Preset, the settings themselves.
    #[serde(deserialize_with = "lenient")]
    pub active: String,
    /// The custom presets, in the order they were made.
    #[serde(deserialize_with = "lenient")]
    pub list: Vec<Preset>,
}

impl PresetSettings {
    /// The preset named `name`.
    pub fn find(&self, name: &str) -> Option<&Preset> {
        self.list.iter().find(|preset| preset.name == name)
    }

    /// The preset in use, or `None` for Default Preset.
    pub fn active(&self) -> Option<&Preset> {
        self.find(&self.active)
    }
}

/// One custom preset.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preset {
    /// Its name, unique among the presets.
    #[serde(deserialize_with = "lenient")]
    pub name: String,
    /// The values it changes, nested as in the settings file (`audio.max_volume_percent`
    /// under `[values.audio]`). Values this build doesn't know are kept as they are.
    #[serde(deserialize_with = "lenient")]
    pub values: toml::Table,
}
