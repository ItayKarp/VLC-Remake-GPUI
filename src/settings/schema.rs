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
use crate::logging::LogLevel;
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
    /// Light, dark, or following the system.
    #[serde(deserialize_with = "lenient")]
    pub theme: ThemeMode,
    /// The accent color.
    #[serde(deserialize_with = "lenient")]
    pub accent: Accent,
    /// Player behavior.
    #[serde(deserialize_with = "lenient")]
    pub player: PlayerSettings,
    /// Audio preferences.
    #[serde(deserialize_with = "lenient")]
    pub audio: AudioSettings,
    /// Video preferences.
    #[serde(deserialize_with = "lenient")]
    pub video: VideoSettings,
    /// Subtitle preferences.
    #[serde(deserialize_with = "lenient")]
    pub subtitles: SubtitleSettings,
    /// Library preferences.
    #[serde(deserialize_with = "lenient")]
    pub library: LibrarySettings,
    /// Privacy preferences.
    #[serde(deserialize_with = "lenient")]
    pub privacy: PrivacySettings,
    /// Custom key bindings.
    #[serde(deserialize_with = "lenient")]
    pub keymap: KeymapSettings,
    /// Logging and crash reports (S0.7).
    #[serde(deserialize_with = "lenient")]
    pub diagnostics: DiagnosticsSettings,
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
    /// Where each window was last placed.
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
    /// Custom key bindings (S0.5).
    KeymapSettings,
}

/// Library preferences (Stage 4, S5.9).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LibrarySettings {
    /// Lets every launch open its own window instead of handing its files to the
    /// running one (S0.6.1). The opposite of "Only allow one VLC window" (S5.9.1).
    #[serde(deserialize_with = "lenient")]
    pub allow_multiple_instances: bool,
}

/// Privacy preferences (Stage 5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrivacySettings {
    /// "Save recently played items" (S5.5.1). Also governs the OS recent documents
    /// list (S0.6.5).
    #[serde(deserialize_with = "lenient")]
    pub save_recently_played: bool,
    /// Hides media file names in the log file, keeping only their extensions
    /// (S0.7.1). The home directory is always replaced by `~`.
    #[serde(deserialize_with = "lenient")]
    pub redact_file_names_in_logs: bool,
}

impl Default for PrivacySettings {
    fn default() -> Self {
        PrivacySettings { save_recently_played: true, redact_file_names_in_logs: false }
    }
}

/// Logging and crash reports (S0.7). `RUST_LOG` overrides the level.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DiagnosticsSettings {
    /// How much goes into the log file.
    #[serde(deserialize_with = "lenient")]
    pub log_level: LogLevel,
}

/// Window geometry per window kind (S0.2.9). Onboarding isn't remembered: it opens
/// centered on the display with the cursor each time.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowState {
    /// The main window.
    #[serde(deserialize_with = "lenient")]
    pub main: WindowGeometry,
    /// The Settings window (Stage 5).
    #[serde(deserialize_with = "lenient")]
    pub settings: WindowGeometry,
}

/// A window's normal (un-maximized) bounds in logical pixels, in GPUI's global
/// coordinate space, and whether it was maximized or fullscreen. A zero size means
/// "never saved".
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowGeometry {
    /// The left edge of the normal (restored) bounds, in logical pixels.
    #[serde(deserialize_with = "lenient")]
    pub x: f32,
    /// The top edge of the normal bounds.
    #[serde(deserialize_with = "lenient")]
    pub y: f32,
    /// The normal width; zero when nothing was saved.
    #[serde(deserialize_with = "lenient")]
    pub width: f32,
    /// The normal height; zero when nothing was saved.
    #[serde(deserialize_with = "lenient")]
    pub height: f32,
    /// Whether the window was maximized or fullscreen.
    #[serde(deserialize_with = "lenient")]
    pub mode: WindowMode,
}

/// How a window was shown when its geometry was saved.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowMode {
    /// A normal window at its saved bounds.
    #[default]
    Windowed,
    /// Maximized; the saved bounds are where it restores to.
    Maximized,
    /// Fullscreen; the saved bounds are where it restores to.
    Fullscreen,
}
