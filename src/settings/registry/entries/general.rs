//! General ("Global Settings"): appearance and diagnostics.

use super::super::def::{Control, SettingDef};
use super::super::value::{Choice, SettingChoice, choice, toggle};
use crate::logging::LogLevel;
use crate::settings::registry::Category;
use crate::theme::{Accent, ThemeMode};

const APPEARANCE: (Category, &str) = (Category::General, "Appearance");
const DIAGNOSTICS: (Category, &str) = (Category::General, "Diagnostics");

impl SettingChoice for ThemeMode {
    const VALUES: &'static [Self] = &[Self::Dark, Self::Light, Self::System];
    const CHOICES: &'static [Choice] = &[
        Choice::new("dark", "Dark"),
        Choice::new("light", "Light"),
        Choice::new("system", "System"),
    ];
}

impl SettingChoice for Accent {
    const VALUES: &'static [Self] = &Accent::ALL;
    const CHOICES: &'static [Choice] = &[
        Choice::new("orange", "Orange"),
        Choice::new("red", "Red"),
        Choice::new("yellow", "Yellow"),
        Choice::new("green", "Green"),
        Choice::new("teal", "Teal"),
        Choice::new("blue", "Blue"),
        Choice::new("purple", "Purple"),
        Choice::new("pink", "Pink"),
        Choice::new("gray", "Gray"),
    ];
}

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

/// The accent swatch's color for a choice id.
fn accent_color(id: &str) -> Option<gpui::Hsla> {
    Accent::from_choice_id(id).map(Accent::color)
}

/// The General page's rows.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    SettingDef::new(
        "theme",
        APPEARANCE,
        "Theme",
        Control::Segmented(ThemeMode::CHOICES),
        choice!(prefs.theme),
    )
    .info("Dark or light windows, or whichever the system uses right now.")
    .keywords(&["appearance", "dark mode", "light mode", "color scheme"]),
    SettingDef::new(
        "accent",
        APPEARANCE,
        "Accent color",
        Control::Swatches { choices: Accent::CHOICES, color: accent_color },
        choice!(prefs.accent),
    )
    .info("The color of selected items, switches, sliders and the progress bar.")
    .keywords(&["highlight", "colour", "tint"]),
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
    .keywords(&["logging", "verbosity", "debug"]),
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
];
