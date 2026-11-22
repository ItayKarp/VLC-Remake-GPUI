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

mod audio;
mod controls;
mod general;
mod interface;
mod miniplayer;
mod playback;
mod subtitles;

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::lenient::lenient;
use super::registry::Category;
use super::stored_path;
use crate::theme::{Accent, ThemeMode};

pub use audio::{AudioSettings, LocalAudio, StartupVolume};
pub use controls::{
    ClickAction, ControlsSettings, DoubleClickAction, HorizontalWheelAction, SkipLength,
    WheelAction,
};
pub use general::{DiagnosticsSettings, InterfaceLanguage, StartupSettings};
pub use interface::{AutoHide, DurationReadout, InterfaceSettings};
pub use miniplayer::MiniplayerCorner;
pub use playback::{PlayerSettings, QueueSiblings, ResumePolicy, WhenFinished};
pub use subtitles::SubtitleSettings;

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
    /// The interface language (S5.3.2).
    #[serde(deserialize_with = "lenient")]
    pub language: InterfaceLanguage,
    /// Light, dark, or following the system.
    #[serde(deserialize_with = "lenient")]
    pub theme: ThemeMode,
    /// The accent color.
    #[serde(deserialize_with = "lenient")]
    pub accent: Accent,
    /// Startup and dialogs (S5.3.3).
    #[serde(deserialize_with = "lenient")]
    pub startup: StartupSettings,
    /// The window and its title bar.
    #[serde(deserialize_with = "lenient")]
    pub interface: InterfaceSettings,
    /// Player behavior.
    #[serde(deserialize_with = "lenient")]
    pub player: PlayerSettings,
    /// The seek keys' skip amounts and the wheel over the video (S5.6).
    #[serde(deserialize_with = "lenient")]
    pub controls: ControlsSettings,
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
    /// Logging, crash reports and libVLC's startup options (S0.7, S5.3.4).
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
    /// Onboarding writes one; the Library settings screen can manage several. The library
    /// indexes each as a video folder (S4.2.1).
    #[serde(with = "stored_path::list")]
    pub media_folders: Vec<PathBuf>,
    /// Folders the library indexes as music (S4.2.7): the OS Music folder, added once.
    #[serde(with = "stored_path::list")]
    pub music_folders: Vec<PathBuf>,
    /// Whether the OS Music folder was offered to the library (S4.2.7, D7), so removing it
    /// later sticks.
    #[serde(deserialize_with = "lenient")]
    pub music_folder_added: bool,
    /// Where each window was last placed.
    #[serde(deserialize_with = "lenient")]
    pub window: WindowState,
    /// Media opened recently, newest first (S1.3.5): URLs as written, local paths encoded
    /// like [`stored_path`] does. Read and written through `opening::recent`.
    #[serde(deserialize_with = "lenient")]
    pub recent_media: Vec<String>,
    /// The output device and last volume (S1.5).
    #[serde(deserialize_with = "lenient")]
    pub audio: LocalAudio,
    /// Where the library was last, per presentation (S4.5.6). Local: routes name folders.
    #[serde(deserialize_with = "lenient")]
    pub library_routes: LibraryRoutes,
    /// The library sections shown as a grid (S4.6.4, S4.7.4), by name, as
    /// `navigation::view` writes them; the others show as lists.
    #[serde(deserialize_with = "lenient")]
    pub library_grid: Vec<String>,
    /// How each library place sorts when it isn't its default (S4.7.2): the sort's name by the
    /// place's route, as `navigation::sort` writes them. Local: routes name folders.
    #[serde(deserialize_with = "lenient")]
    pub library_sorts: BTreeMap<String, String>,
    /// The corner full library mode's miniplayer card was last docked in (S4.10.5).
    #[serde(deserialize_with = "lenient")]
    pub miniplayer_corner: MiniplayerCorner,
    /// The page the Settings window last showed (S5.2.2).
    #[serde(deserialize_with = "lenient")]
    pub settings_category: Category,
    /// "Log file" (S5.3.4): where the log is written, as typed; empty means the default,
    /// `logs/vlc.log` next to this file. Local: it is a path on this machine.
    #[serde(deserialize_with = "lenient")]
    pub log_file: String,
}

/// The library's last route in each presentation, as `navigation::stored` writes routes. Empty
/// means Home.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LibraryRoutes {
    /// The sidebar overlay over the player.
    #[serde(deserialize_with = "lenient")]
    pub overlay: String,
    /// Full library mode.
    #[serde(deserialize_with = "lenient")]
    pub full: String,
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
    /// Video preferences (Stage 7).
    VideoSettings,
    /// Custom key bindings (S0.5).
    KeymapSettings,
}

/// Library preferences (Stage 4, S5.9).
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent switches, each its own row in Settings → Library"
)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LibrarySettings {
    /// Lets every launch open its own window instead of handing its files to the
    /// running one (S0.6.1). The opposite of "Only allow one VLC window" (S5.9.1).
    #[serde(deserialize_with = "lenient")]
    pub allow_multiple_instances: bool,
    /// "When opening a folder: Include subfolders" (S5.9): whether opening a folder, or a
    /// file's folder (S3.1.3), takes the media in its subfolders too.
    #[serde(deserialize_with = "lenient")]
    pub include_subfolders: bool,
    /// "Show hidden files" (S5.9.1): whether the library indexes dot files and files the OS
    /// marks hidden or system (S4.2.2).
    #[serde(deserialize_with = "lenient")]
    pub show_hidden_files: bool,
    /// "Add to the playlist instead of replacing it" (S4.10.6): whether opening a library item
    /// while something plays adds it to the end of the queue rather than replacing the queue.
    #[serde(deserialize_with = "lenient")]
    pub add_instead_of_replacing: bool,
}

impl Default for LibrarySettings {
    fn default() -> Self {
        Self {
            allow_multiple_instances: false,
            include_subfolders: true,
            show_hidden_files: false,
            add_instead_of_replacing: false,
        }
    }
}

/// Privacy preferences (Stage 5).
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent switches, each its own row in Settings → Privacy"
)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrivacySettings {
    /// "Save recently played items" (S5.5.1). Also governs the OS recent documents
    /// list (S0.6.5) and the playback history (S1.7): off, nothing is written to either.
    #[serde(deserialize_with = "lenient")]
    pub save_recently_played: bool,
    /// Hides media file names in the log file, keeping only their extensions
    /// (S0.7.1). The home directory is always replaced by `~`.
    #[serde(deserialize_with = "lenient")]
    pub redact_file_names_in_logs: bool,
    /// How many items Open Recent keeps (S1.3.5); `0` keeps none.
    #[serde(deserialize_with = "lenient")]
    pub recent_media_count: u32,
    /// "Remember the playback position" (S5.5.1): save where media was left, so it can resume
    /// (S1.7). Plays are still counted while history is saved.
    #[serde(deserialize_with = "lenient")]
    pub remember_playback_position: bool,
    /// "Fetch titles and cover art from the internet" (S5.5, S4.3.4): whether the library may
    /// look covers up online for media without one. Off by default: nothing about the user's
    /// files leaves the machine unless they turn it on.
    #[serde(deserialize_with = "lenient")]
    pub fetch_online_metadata: bool,
}

/// How many items Open Recent keeps by default.
pub const DEFAULT_RECENT_MEDIA_COUNT: u32 = 10;

impl Default for PrivacySettings {
    fn default() -> Self {
        PrivacySettings {
            save_recently_played: true,
            redact_file_names_in_logs: false,
            recent_media_count: DEFAULT_RECENT_MEDIA_COUNT,
            remember_playback_position: true,
            fetch_online_metadata: false,
        }
    }
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
