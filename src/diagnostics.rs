//! Diagnostics (S0.7, S5.3.4): starts logging and the crash reporter before anything else,
//! keeps logging in step with the settings, sets libVLC's startup options, and builds "Copy Debug
//! Info".
//!
//! Nothing here is ever uploaded: crash reports and logs stay in the `logs/`
//! folder until the user chooses to share them (S9.6, `docs/privacy.md`).

pub mod crash;
pub mod debug_info;
pub mod report_issue;
pub mod scrub;
pub mod system;

use std::path::{Path, PathBuf};

use gpui::App;

use crate::engine::libvlc::options as libvlc_options;
use crate::logging::{self, LogFile, LogOptions};
use crate::settings::{self, Locations, Preferences, Settings, SettingsStore};

/// Starts logging and installs the panic hook, and sets the player's libVLC options. Runs before
/// GPUI, so it reads the preferences straight from disk; the log file the user picked, which is
/// machine-local state, is used once the settings load ([`follow_settings`]).
pub fn init() {
    let logs = logs_dir();
    let prefs = settings::peek();
    logging::init(&log_options(&prefs, "", logs.as_deref()));
    logging::set_redact_file_names(prefs.privacy.redact_file_names_in_logs);
    crash::set_developer_mode(prefs.diagnostics.developer_mode);
    crash::install(logs);
    libvlc_options::set_startup(
        &prefs.diagnostics.extra_options,
        prefs.diagnostics.collect_statistics,
    );
    tracing::info!("VLC {} starting on {}", system::VERSION, system::os());
}

/// The folder for logs and crash reports, if the app has a place to keep files.
pub fn logs_dir() -> Option<PathBuf> {
    Locations::resolve().map(|locations| locations.logs())
}

/// Re-applies the log settings and file-name redaction whenever settings change.
pub fn follow_settings(cx: &mut App) {
    let logs = logs_dir();
    apply(settings::get(cx), logs.as_deref());
    cx.observe_global::<SettingsStore>(move |cx| apply(settings::get(cx), logs.as_deref()))
        .detach();
}

fn apply(settings: &Settings, logs: Option<&Path>) {
    let prefs = &settings.prefs;
    logging::configure(&log_options(prefs, &settings.local.log_file, logs));
    logging::set_redact_file_names(prefs.privacy.redact_file_names_in_logs);
    crash::set_developer_mode(prefs.diagnostics.developer_mode);
}

/// What the settings ask of logging. `log_file` is the "Log file" setting; empty, or without a
/// place for logs, the default file in `logs` is used.
fn log_options(prefs: &Preferences, log_file: &str, logs: Option<&Path>) -> LogOptions {
    let diagnostics = &prefs.diagnostics;
    LogOptions {
        level: diagnostics.log_level,
        minor_warnings: diagnostics.show_minor_warnings,
        file: diagnostics.save_log.then(|| log_file_target(log_file, logs)).flatten(),
        format: diagnostics.log_format,
    }
}

/// The picked log file with the default as its fallback, or the default alone.
fn log_file_target(log_file: &str, logs: Option<&Path>) -> Option<LogFile> {
    let default = logs.map(logging::default_file);
    let picked = Some(log_file.trim()).filter(|text| !text.is_empty()).map(PathBuf::from);
    match (picked, default) {
        (Some(path), Some(fallback)) => Some(LogFile { path, fallback }),
        (Some(path), None) | (None, Some(path)) => Some(LogFile { fallback: path.clone(), path }),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_log_file_without_save_a_log() {
        let mut prefs = Preferences::default();
        prefs.diagnostics.save_log = false;
        assert_eq!(log_options(&prefs, "/tmp/a.log", Some(Path::new("/logs"))).file, None);
    }

    #[test]
    fn a_picked_file_falls_back_to_the_default() {
        let prefs = Preferences::default();
        let file = log_options(&prefs, " /tmp/a.log ", Some(Path::new("/logs"))).file.unwrap();
        assert_eq!(file.path, PathBuf::from("/tmp/a.log"));
        assert_eq!(file.fallback, Path::new("/logs").join("vlc.log"));
        let default = log_options(&prefs, "", Some(Path::new("/logs"))).file.unwrap();
        assert_eq!(default.path, default.fallback);
    }
}
