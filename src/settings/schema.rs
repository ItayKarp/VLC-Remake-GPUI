//! The typed settings schema.
//!
//! Rules for every field added here (later stages fill the empty sections):
//! - Annotate it with `#[serde(deserialize_with = "lenient")]` (or a lenient custom
//!   deserializer) so one bad value falls back to its default instead of failing the file.
//! - The struct carries `#[serde(default)]`, so missing keys take their defaults.
//! - Don't use `Option` for persisted values: `None` isn't serialized, so the old value
//!   would survive in the file when the document is merged on save.
//! - Machine-specific data (paths, window geometry, device IDs) goes in [`LocalState`],
//!   everything that should follow the user between machines in [`Preferences`].

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::lenient::lenient;
use super::stored_path;
use crate::theme::{Accent, ThemeMode};

/// Everything the app persists, as one in-memory value.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Settings {
    /// Roams with the user (`settings.toml` in the config dir).
    pub prefs: Preferences,
    /// Stays on this machine (`local.toml` in the local data dir).
    pub local: LocalState,
}

/// User preferences that follow the user between machines.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    #[serde(deserialize_with = "lenient")]
    pub theme: ThemeMode,
    #[serde(deserialize_with = "lenient")]
    pub accent: Accent,
    #[serde(deserialize_with = "lenient")]
    pub player: PlayerSettings,
    #[serde(deserialize_with = "lenient")]
    pub audio: AudioSettings,
    #[serde(deserialize_with = "lenient")]
    pub video: VideoSettings,
    #[serde(deserialize_with = "lenient")]
    pub subtitles: SubtitleSettings,
    #[serde(deserialize_with = "lenient")]
    pub library: LibrarySettings,
    #[serde(deserialize_with = "lenient")]
    pub privacy: PrivacySettings,
    #[serde(deserialize_with = "lenient")]
    pub keymap: KeymapSettings,
}

/// State that only makes sense on this machine.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LocalState {
    /// Local rather than roaming: onboarding picks a media folder, which is
    /// machine-specific, so a new machine should run onboarding again.
    #[serde(deserialize_with = "lenient")]
    pub onboarding_complete: bool,
    /// Onboarding writes one; the Library settings screen can manage several.
    #[serde(with = "stored_path::list")]
    pub media_folders: Vec<PathBuf>,
    #[serde(deserialize_with = "lenient")]
    pub window: WindowState,
}

macro_rules! empty_sections {
    ($($(#[$meta:meta])* $name:ident),* $(,)?) => {$(
        $(#[$meta])*
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct $name {}
    )*};
}

empty_sections! {
    /// Player behavior (Stage 2).
    PlayerSettings,
    /// Audio preferences (Stage 1/7). The output device ID is local, not here.
    AudioSettings,
    /// Video preferences (Stage 7).
    VideoSettings,
    /// Subtitle preferences (Stage 6).
    SubtitleSettings,
    /// Library preferences (Stage 4).
    LibrarySettings,
    /// Privacy preferences (Stage 5).
    PrivacySettings,
    /// Custom key bindings (S0.5).
    KeymapSettings,
    /// Window geometry per window kind (S0.2.9).
    WindowState,
}
