//! General ("Global Settings"): the Diagnostics section and its advanced rows (S0.7, S5.3.4).

use std::path::Path;

use super::super::def::{ApplyMode, Condition, Control, PathPick, SettingDef};
use super::super::value::{Binding, Choice, SettingChoice, Value, choice, text, toggle};
use crate::engine::libvlc::options as libvlc_options;
use crate::logging::{self, LogFormat, LogLevel};
use crate::settings::registry::Category;

const DIAGNOSTICS: (Category, &str) = (Category::General, "Diagnostics");
/// The key the log rows depend on.
const SAVE_LOG: &str = "diagnostics.save_log";

impl SettingChoice for LogLevel {
    const VALUES: &'static [Self] =
        &[Self::Error, Self::Warn, Self::Info, Self::Debug, Self::Trace];
    const CHOICES: &'static [Choice] = &[
        Choice::new("error", "Errors only"),
        Choice::new("warn", "Errors and warnings"),
        Choice::new("info", "Notable events"),
        Choice::new("debug", "Details for bug reports"),
        Choice::new("trace", "Everything"),
    ];
}

impl SettingChoice for LogFormat {
    const VALUES: &'static [Self] = &[Self::Plain, Self::Json];
    const CHOICES: &'static [Choice] =
        &[Choice::new("plain", "Plain text"), Choice::new("json", "JSON lines")];
}

/// A log file path VLC can use; empty means the default file.
fn log_file(value: &Value) -> Result<(), String> {
    match value {
        Value::Text(text) if !text.trim().is_empty() => logging::check_file(Path::new(text.trim())),
        _ => Ok(()),
    }
}

/// libVLC options VLC can start with.
fn extra_options(value: &Value) -> Result<(), String> {
    match value {
        Value::Text(text) => libvlc_options::check(text),
        _ => Ok(()),
    }
}

/// Opens the component gallery, an unstable developer tool.
fn open_gallery(window: &mut gpui::Window, cx: &mut gpui::App) {
    let display = window.display(cx).map(|display| display.id());
    if let Err(err) = crate::app::open_gallery(display, cx) {
        tracing::error!("couldn't open the component gallery: {err:#}");
    }
}

/// The General page's Diagnostics rows.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    SettingDef::new(
        "diagnostics.developer_mode",
        DIAGNOSTICS,
        "Developer mode",
        Control::Toggle,
        toggle!(prefs.diagnostics.developer_mode),
    )
    .info(
        "Shows unstable tools for people working on VLC, marked as such. Crash reports made \
         meanwhile say so, and are left out of crash statistics.",
    )
    .keywords(&["debug", "unstable", "experimental"]),
    SettingDef::new(
        SAVE_LOG,
        DIAGNOSTICS,
        "Save a log",
        Control::Toggle,
        toggle!(prefs.diagnostics.save_log),
    )
    .info("Keeps a record of what VLC does in a file, to attach to a bug report.")
    .keywords(&["logging", "log file", "record"]),
    SettingDef::new(
        "log_file",
        DIAGNOSTICS,
        "Log file",
        Control::Path(PathPick::NewFile { name: "vlc.log" }),
        text!(local.log_file),
    )
    .info(
        "Where the log is written. Leave it empty for the usual place, next to VLC's settings. \
         If the file can't be written, the usual place is used instead.",
    )
    .validate(log_file)
    .depends_on(SAVE_LOG, Condition::IsOn)
    .keywords(&["logging", "path", "location", "folder"]),
    SettingDef::new(
        "diagnostics.log_level",
        DIAGNOSTICS,
        "Log details",
        Control::Dropdown(LogLevel::CHOICES),
        choice!(prefs.diagnostics.log_level),
    )
    .info(
        "How much VLC writes to its log file. More detail helps with bug reports but makes the \
         file grow faster.",
    )
    .depends_on(SAVE_LOG, Condition::IsOn)
    .keywords(&["logging", "verbosity", "debug"]),
    SettingDef::new(
        "diagnostics.show_minor_warnings",
        DIAGNOSTICS,
        "Show minor warnings",
        Control::Toggle,
        toggle!(prefs.diagnostics.show_minor_warnings),
    )
    .info(
        "Also logs warnings from the parts VLC is built on, such as its graphics. They rarely \
         mean anything is wrong.",
    )
    .keywords(&["logging", "verbose", "noise"])
    .advanced(),
    SettingDef::new(
        "diagnostics.extra_options",
        DIAGNOSTICS,
        "Additional command line options",
        Control::Text,
        text!(prefs.diagnostics.extra_options),
    )
    .info(
        "libVLC options the player starts with, such as --network-caching=3000. Use quotes \
         around a value with spaces. Takes effect when VLC restarts.",
    )
    .validate(extra_options)
    .applies(ApplyMode::Restart)
    .keywords(&["arguments", "flags", "libvlc", "cli"])
    .advanced(),
    SettingDef::new(
        "diagnostics.log_format",
        DIAGNOSTICS,
        "Log format",
        Control::Dropdown(LogFormat::CHOICES),
        choice!(prefs.diagnostics.log_format),
    )
    .info("Plain text to read, or one JSON object per line for tools that read logs.")
    .depends_on(SAVE_LOG, Condition::IsOn)
    .keywords(&["logging", "json", "structured"])
    .advanced(),
    SettingDef::new(
        "diagnostics.collect_statistics",
        DIAGNOSTICS,
        "Collect playback statistics",
        Control::Toggle,
        toggle!(prefs.diagnostics.collect_statistics),
    )
    .info(
        "Counts bitrates and dropped or lost pictures while playing. Turning it off saves a \
         little work. Takes effect when VLC restarts.",
    )
    .applies(ApplyMode::Restart)
    .keywords(&["stats", "bitrate", "dropped frames"])
    .advanced(),
    SettingDef::new(
        "privacy.redact_file_names_in_logs",
        DIAGNOSTICS,
        "Hide file names in the log",
        Control::Toggle,
        toggle!(prefs.privacy.redact_file_names_in_logs),
    )
    .info(
        "Writes only the extension of each media file to the log, so a shared log doesn't show \
         what you watch.",
    )
    .keywords(&["privacy", "redact", "anonymize"])
    .advanced(),
    SettingDef::new(
        "component_gallery",
        DIAGNOSTICS,
        "Component gallery (unstable)",
        Control::Button { label: "Open", action: open_gallery },
        Binding::ACTION,
    )
    .info(
        "A window showing every control VLC is built from, for developers. It may change or break.",
    )
    .depends_on("diagnostics.developer_mode", Condition::IsOn)
    .keywords(&["developer", "components", "widgets", "debug"])
    .advanced(),
];
