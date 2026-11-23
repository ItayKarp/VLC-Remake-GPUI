//! Player profiles (S8.8): Movies, Shows, Music, Work, each a player layout and a setting preset
//! (D12). `settings::profiles` switches between them.
//!
//! The profile in use keeps its layout in `player_layout` and its preset in `presets.active`,
//! where everything reads them; its entry here is brought up to date when another is chosen. As
//! with presets, the profiles are a list of tables so a deleted one doesn't survive the save's
//! merge.

use serde::{Deserialize, Serialize};

use super::super::lenient::lenient;
use super::StoredLayout;

/// The profiles the user has, and the one in use.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileSettings {
    /// The name of the profile in use; empty, or a name no profile has, for the first.
    #[serde(deserialize_with = "lenient")]
    pub active: String,
    /// The profiles, in menu order. Empty until one is changed, which means the designed
    /// four.
    #[serde(deserialize_with = "lenient")]
    pub list: Vec<Profile>,
}

/// One profile.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    /// Its name, unique among the profiles.
    #[serde(deserialize_with = "lenient")]
    pub name: String,
    /// The name of its setting preset (S5.11); empty for Default Preset.
    #[serde(deserialize_with = "lenient")]
    pub preset: String,
    /// Its player layout (S8.7); empty for the designed one.
    #[serde(deserialize_with = "lenient")]
    pub layout: StoredLayout,
    /// The media it is switched to for by itself, if any (S8.8.3).
    #[serde(deserialize_with = "lenient")]
    pub automatic: Automatic,
}

/// What media switches to a profile by itself (S8.8.3). Each kind belongs to one profile at
/// most.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Automatic {
    /// Only the user switches to it.
    #[default]
    Never,
    /// Audio-only media, which plays in music mode (S8.1).
    Music,
    /// Media with moving pictures.
    Video,
}
