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
mod keymap;
mod library;
mod live;
mod miniplayer;
mod network;
mod performance;
mod playback;
mod presets;
mod privacy;
mod recording;
mod subtitles;
mod video;

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
pub use interface::{
    AutoHide, DurationReadout, FitVideo, InterfaceSettings, NowPlayingNotification,
};
pub use keymap::{CustomBinding, KeymapSettings};
pub use library::{ArtworkSource, LibrarySettings};
pub use live::LiveState;
pub use miniplayer::MiniplayerCorner;
pub use network::{Buffering, Caching, CipherPreference, NetworkSettings, proxy_url};
pub use performance::{
    ClockSynchronisation, Decoder, Encoder, HardwareDecoding, PerformanceSettings, PostProcessing,
    SkipLoopFilter,
};
pub use playback::{PlayerSettings, QueueSiblings, ResumePolicy, VideoQuality, WhenFinished};
pub use presets::{Preset, PresetSettings};
pub use privacy::PrivacySettings;
pub use recording::{CastQuality, EncodingPreset, RecordingSettings, Tuning};
pub use subtitles::{SizeValues, SubtitleArea, SubtitleSettings};
pub use video::{PictureSettings, VideoSettings};

/// Everything the app persists, as one in-memory value.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Settings {
    /// Roams with the user (`settings.toml` in the config dir).
    pub prefs: Preferences,
    /// Stays on this machine (`local.toml` in the local data dir).
    pub local: LocalState,
    /// State that lives elsewhere but that Settings rows show and edit; never saved.
    pub live: LiveState,
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
    /// Buffering, the proxy and TLS (S5.10.1).
    #[serde(deserialize_with = "lenient")]
    pub network: NetworkSettings,
    /// Decoding, priority and the clock (S5.10.2).
    #[serde(deserialize_with = "lenient")]
    pub performance: PerformanceSettings,
    /// Recording, casting and streaming out (S5.10.3).
    #[serde(deserialize_with = "lenient")]
    pub recording: RecordingSettings,
    /// Custom key bindings (S5.6.3).
    #[serde(deserialize_with = "lenient")]
    pub keymap: KeymapSettings,
    /// Logging, crash reports and libVLC's startup options (S0.7, S5.3.4).
    #[serde(deserialize_with = "lenient")]
    pub diagnostics: DiagnosticsSettings,
    /// Setting presets over the Playback, Video, Audio and Subtitles pages (S5.11). Change
    /// them through `settings::presets`, which keeps the sections they cover in step.
    #[serde(deserialize_with = "lenient")]
    pub presets: PresetSettings,
}

/// State that only makes sense on this machine.
#[allow(
    clippy::struct_excessive_bools,
    reason = "unrelated flags of different features, not one state machine"
)]
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
    /// The player panel's effect sections left expanded (S7.1.2), by their ids.
    #[serde(deserialize_with = "lenient")]
    pub expanded_effects: Vec<String>,
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
    /// "Open fullscreen on" (S5.4.1): the display's id as `app::screens` writes it; empty means
    /// the screen the window is on. Local: displays belong to this machine.
    #[serde(deserialize_with = "lenient")]
    pub fullscreen_display: String,
    /// "Skin file" (S5.4.4), as typed. Local: it is a path on this machine.
    #[serde(deserialize_with = "lenient")]
    pub skin_file: String,
    /// "Save recordings to" (S5.10.3), as typed; empty means the OS Videos folder. Local: it is
    /// a path on this machine.
    #[serde(deserialize_with = "lenient")]
    pub recordings_folder: String,
    /// "Rescan plugins" (S5.10.2) was asked for: the next launch has libVLC look for its plugins
    /// again instead of trusting its cache, then clears this.
    #[serde(deserialize_with = "lenient")]
    pub rescan_plugins: bool,
    /// Media VLC closed unexpectedly on while the GPU decoded it (S5.10.2), as paths or URLs:
    /// they decode in software. Local: the GPU and the paths belong to this machine.
    #[serde(deserialize_with = "lenient")]
    pub software_decoding: Vec<String>,
    /// How many times VLC closed unexpectedly while each GPU and driver decoded video; at
    /// `app::hardware_fallback`'s limit, hardware decoding is off for it.
    #[serde(deserialize_with = "lenient")]
    pub hardware_decoding_crashes: BTreeMap<String, u32>,
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
    /// The video's own window, when video doesn't play in the main window (S5.4.1).
    #[serde(deserialize_with = "lenient")]
    pub video: WindowGeometry,
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
