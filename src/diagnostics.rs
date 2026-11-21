//! Diagnostics (S0.7): starts logging and the crash reporter before anything else,
//! keeps logging in step with the settings, and builds "Copy Debug Info".
//!
//! Nothing here is ever uploaded: crash reports and logs stay in the `logs/`
//! folder until the user chooses to share them (S9.6).

pub mod crash;
pub mod debug_info;
pub mod system;

use std::path::PathBuf;

use gpui::App;

use crate::logging;
use crate::settings::{self, Locations, Preferences, SettingsStore};

/// Starts logging and installs the panic hook. Runs before GPUI, so it reads the
/// preferences straight from disk.
pub fn init() {
    let logs = logs_dir();
    let prefs = settings::peek();
    logging::init(prefs.diagnostics.log_level, logs.as_deref());
    apply(&prefs);
    crash::install(logs);
    tracing::info!("VLC {} starting on {}", system::VERSION, system::os());
}

/// The folder for logs and crash reports, if the app has a place to keep files.
pub fn logs_dir() -> Option<PathBuf> {
    Locations::resolve().map(|locations| locations.logs())
}

/// Re-applies the log level and file-name redaction whenever settings change.
pub fn follow_settings(cx: &mut App) {
    apply(&settings::get(cx).prefs);
    cx.observe_global::<SettingsStore>(|cx| apply(&settings::get(cx).prefs)).detach();
}

fn apply(prefs: &Preferences) {
    logging::set_level(prefs.diagnostics.log_level);
    logging::set_redact_file_names(prefs.privacy.redact_file_names_in_logs);
}
