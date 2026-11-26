//! General ("Global Settings"): the Diagnostics section and its advanced rows (S0.7, S5.3.4).

use std::path::Path;

use super::super::def::{ApplyMode, Condition, Control, PathPick, SettingDef};
use super::super::value::{Binding, Choice, SettingChoice, Value, choice, text, toggle};
use crate::engine::libvlc::options as libvlc_options;
use crate::logging::{self, LogFormat, LogLevel};
use crate::settings::registry::Category;

const DIAGNOSTICS: (Category, &str) = (Category::General, tr_noop!("Diagnostics"));
/// The key the log rows depend on.
const SAVE_LOG: &str = "diagnostics.save_log";

impl SettingChoice for LogLevel {
    const VALUES: &'static [Self] =
        &[Self::Error, Self::Warn, Self::Info, Self::Debug, Self::Trace];
    const CHOICES: &'static [Choice] = &[
        Choice::new("error", tr_noop!("Errors only")),
        Choice::new("warn", tr_noop!("Errors and warnings")),
        Choice::new("info", tr_noop!("Notable events")),
        Choice::new("debug", tr_noop!("Details for bug reports")),
        Choice::new("trace", tr_noop!("Everything")),
    ];
}

impl SettingChoice for LogFormat {
    const VALUES: &'static [Self] = &[Self::Plain, Self::Json];
    const CHOICES: &'static [Choice] = &[
        Choice::new("plain", tr_noop!("Plain text")),
        Choice::new("json", tr_noop!("JSON lines")),
    ];
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
        tr_noop!("Developer mode"),
        Control::Toggle,
        toggle!(prefs.diagnostics.developer_mode),
    )
    .info(tr_noop!(
        "Shows unstable tools for people working on VLC, marked as such. Crash reports made \
         meanwhile say so, and are left out of crash statistics."
    ))
    .keywords(&["debug", "unstable", "experimental"]),
    SettingDef::new(
        SAVE_LOG,
        DIAGNOSTICS,
        tr_noop!("Save a log"),
        Control::Toggle,
        toggle!(prefs.diagnostics.save_log),
    )
    .info(tr_noop!("Keeps a record of what VLC does in a file, to attach to a bug report."))
    .keywords(&["logging", "log file", "record"]),
    SettingDef::new(
        "log_file",
        DIAGNOSTICS,
        tr_noop!("Log file"),
        Control::Path(PathPick::NewFile { name: "vlc.log" }),
        text!(local.log_file),
    )
    .info(tr_noop!(
        "Where the log is written. Leave it empty for the usual place, next to VLC's settings. If \
         the file can't be written, the usual place is used instead."
    ))
    .validate(log_file)
    .depends_on(SAVE_LOG, Condition::IsOn)
    .keywords(&["logging", "path", "location", "folder"]),
    SettingDef::new(
        "diagnostics.log_level",
        DIAGNOSTICS,
        tr_noop!("Log details"),
        Control::Dropdown(LogLevel::CHOICES),
        choice!(prefs.diagnostics.log_level),
    )
    .info(tr_noop!(
        "How much VLC writes to its log file. More detail helps with bug reports but makes the \
         file grow faster."
    ))
    .depends_on(SAVE_LOG, Condition::IsOn)
    .keywords(&["logging", "verbosity", "debug"]),
    SettingDef::new(
        "diagnostics.show_minor_warnings",
        DIAGNOSTICS,
        tr_noop!("Show minor warnings"),
        Control::Toggle,
        toggle!(prefs.diagnostics.show_minor_warnings),
    )
    .info(tr_noop!(
        "Also logs warnings from the parts VLC is built on, such as its graphics. They rarely mean \
         anything is wrong."
    ))
    .keywords(&["logging", "verbose", "noise"])
    .advanced(),
    SettingDef::new(
        "diagnostics.extra_options",
        DIAGNOSTICS,
        tr_noop!("Additional command line options"),
        Control::Text,
        text!(prefs.diagnostics.extra_options),
    )
    .info(tr_noop!(
        "libVLC options the player starts with, such as --network-caching=3000. Use quotes around \
         a value with spaces. Takes effect when VLC restarts."
    ))
    .validate(extra_options)
    .applies(ApplyMode::Restart)
    .keywords(&["arguments", "flags", "libvlc", "cli"])
    .advanced(),
    SettingDef::new(
        "diagnostics.log_format",
        DIAGNOSTICS,
        tr_noop!("Log format"),
        Control::Dropdown(LogFormat::CHOICES),
        choice!(prefs.diagnostics.log_format),
    )
    .info(tr_noop!("Plain text to read, or one JSON object per line for tools that read logs."))
    .depends_on(SAVE_LOG, Condition::IsOn)
    .keywords(&["logging", "json", "structured"])
    .advanced(),
    SettingDef::new(
        "diagnostics.collect_statistics",
        DIAGNOSTICS,
        tr_noop!("Collect playback statistics"),
        Control::Toggle,
        toggle!(prefs.diagnostics.collect_statistics),
    )
    .info(tr_noop!(
        "Counts bitrates and dropped or lost pictures while playing, for Show Statistics. The \
         counts stay on this computer. Takes effect when VLC restarts."
    ))
    .applies(ApplyMode::Restart)
    .keywords(&["stats", "bitrate", "dropped frames", "telemetry", "privacy"])
    .advanced(),
    SettingDef::new(
        "component_gallery",
        DIAGNOSTICS,
        tr_noop!("Component gallery (unstable)"),
        Control::Button { label: tr_noop!("Open"), action: open_gallery },
        Binding::ACTION,
    )
    .info(tr_noop!(
        "A window showing every control VLC is built from, for developers. It may change or break."
    ))
    .depends_on("diagnostics.developer_mode", Condition::IsOn)
    .keywords(&["developer", "components", "widgets", "debug"])
    .advanced(),
];
