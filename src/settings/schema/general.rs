//! General preferences (Settings → General, "Global Settings"; S5.3): the interface language, what
//! happens at startup, and diagnostics (S0.7).

use serde::{Deserialize, Serialize};

use super::super::lenient::lenient;
use crate::logging::{LogFormat, LogLevel};

/// The language the interface is drawn in (S5.3.2, S9.2.4). Only English is translated so far;
/// the pseudo-locales are made-up translations for checking long text and mirroring, listed
/// only in debug builds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InterfaceLanguage {
    /// The OS language, where VLC has it; English otherwise.
    #[default]
    System,
    /// English.
    English,
    /// Accented English, about 40% longer (S9.2.3).
    #[serde(rename = "pseudo-accented")]
    PseudoAccented,
    /// English in right-to-left paragraphs, with the layout mirrored (S9.2.2).
    #[serde(rename = "pseudo-rtl")]
    PseudoRtl,
}

/// What happens at startup and how much VLC asks (S5.3.3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StartupSettings {
    /// "Show confirmation and error dialogs": ask before discarding or deleting things. Off, the
    /// action happens at once (a deleted playlist can still be undone from its toast).
    #[serde(deserialize_with = "lenient")]
    pub show_dialogs: bool,
    /// "Check for updates automatically" (S9.5). Off by default, as the design shows.
    #[serde(deserialize_with = "lenient")]
    pub check_for_updates: bool,
}

impl Default for StartupSettings {
    fn default() -> Self {
        Self { show_dialogs: true, check_for_updates: false }
    }
}

/// Logging, crash reports and libVLC's startup options (S0.7, S5.3.4). `RUST_LOG` overrides the
/// level. Where the log goes is machine-specific, so it is [`super::LocalState::log_file`].
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent switches, each its own row in Settings → General"
)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DiagnosticsSettings {
    /// "Developer mode": shows unstable tools, labeled as such, and marks crash reports so crash
    /// statistics can leave them out.
    #[serde(deserialize_with = "lenient")]
    pub developer_mode: bool,
    /// "Save a log": whether events are written to a log file as well as to stderr.
    #[serde(deserialize_with = "lenient")]
    pub save_log: bool,
    /// How much goes into the log file.
    #[serde(deserialize_with = "lenient")]
    pub log_level: LogLevel,
    /// "Show minor warnings": also log the warnings of the components VLC is built on (GPUI);
    /// off, only their errors.
    #[serde(deserialize_with = "lenient")]
    pub show_minor_warnings: bool,
    /// "Additional command line options": libVLC options added when the player starts, as typed
    /// (`--network-caching=3000`). Checked before they are saved, and never run by a shell.
    #[serde(deserialize_with = "lenient")]
    pub extra_options: String,
    /// "Log format": plain text or one JSON object per line.
    #[serde(deserialize_with = "lenient")]
    pub log_format: LogFormat,
    /// "Collect playback statistics": libVLC counts bitrates and lost pictures while playing.
    #[serde(deserialize_with = "lenient")]
    pub collect_statistics: bool,
}

impl Default for DiagnosticsSettings {
    fn default() -> Self {
        Self {
            developer_mode: false,
            save_log: true,
            log_level: LogLevel::default(),
            show_minor_warnings: false,
            extra_options: String::new(),
            log_format: LogFormat::default(),
            collect_statistics: true,
        }
    }
}
