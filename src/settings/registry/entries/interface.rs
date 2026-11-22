//! Interface: the title bar and when the controls hide.

use super::super::def::{Condition, Control, SettingDef};
use super::super::rules::trim;
use super::super::value::{Binding, Choice, SettingChoice, Value, choice, toggle};
use crate::settings::registry::Category;
use crate::settings::{AutoHide, DurationReadout};

const WINDOW: (Category, &str) = (Category::Interface, "Window");
const CONTROLS: (Category, &str) = (Category::Interface, "Playback Controls");
/// Milliseconds in the second "Hide after" is shown in.
const MS_PER_SECOND: f64 = 1000.;

impl SettingChoice for AutoHide {
    const VALUES: &'static [Self] = &[Self::InFullscreenOnly, Self::Always, Self::Never];
    const CHOICES: &'static [Choice] = &[
        Choice::new("in_fullscreen_only", "In fullscreen only"),
        Choice::new("always", "Always"),
        Choice::new("never", "Never"),
    ];
}

impl SettingChoice for DurationReadout {
    const VALUES: &'static [Self] = &[Self::Total, Self::Remaining];
    const CHOICES: &'static [Choice] =
        &[Choice::new("total", "Total length"), Choice::new("remaining", "Time left")];
}

/// "Hide after" in seconds, stored in milliseconds.
const HIDE_AFTER: Binding = Binding {
    get: |s| Value::Number(f64::from(s.prefs.interface.hide_controls_after_ms) / MS_PER_SECOND),
    set: |s, value| {
        if let Value::Number(seconds) = value {
            s.prefs.interface.hide_controls_after_ms = (seconds * MS_PER_SECOND).round() as u32;
        }
    },
};

/// A delay as the slider labels it: "3 s".
fn seconds(value: f64) -> String {
    format!("{} s", trim(value))
}

/// The Interface page's rows.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    SettingDef::new(
        "interface.show_file_name_in_title_bar",
        WINDOW,
        "Show the file name in the title bar",
        Control::Toggle,
        toggle!(prefs.interface.show_file_name_in_title_bar),
    )
    .info(
        "Names the media playing in the title bar and the window's title. Off, they always say \
         VLC Media Player.",
    )
    .keywords(&["window title", "caption"]),
    SettingDef::new(
        "interface.auto_hide_controls",
        CONTROLS,
        "Auto-hide the controls",
        Control::Dropdown(AutoHide::CHOICES),
        choice!(prefs.interface.auto_hide_controls),
    )
    .info(
        "Where the title and control bars hide while video plays and the pointer rests. They \
         come back when you move the pointer.",
    )
    .keywords(&["hide toolbar", "hide bars", "fullscreen controls"]),
    SettingDef::new(
        "interface.hide_controls_after_ms",
        CONTROLS,
        "Hide after",
        Control::Slider { marks: None, format: seconds },
        HIDE_AFTER,
    )
    .info("How long the pointer rests before the controls hide.")
    .range(0.5, 10., 0.5)
    .depends_on("interface.auto_hide_controls", Condition::IsNot("never"))
    .keywords(&["delay", "timeout"]),
    SettingDef::new(
        "interface.show_controls_in_fullscreen",
        CONTROLS,
        "Show controls in fullscreen",
        Control::Toggle,
        toggle!(prefs.interface.show_controls_in_fullscreen),
    )
    .info(
        "Shows the controls when you move the pointer in fullscreen. Off, only the \"Show \
         interface controls in fullscreen\" key (I) brings them up.",
    ),
    SettingDef::new(
        "interface.keep_controls_while_paused",
        CONTROLS,
        "Keep the controls while paused",
        Control::Toggle,
        toggle!(prefs.interface.keep_controls_while_paused),
    )
    .info("Keeps the title and control bars up while playback is paused.")
    .advanced(),
    SettingDef::new(
        "interface.duration_readout",
        CONTROLS,
        "Time readout shows",
        Control::Dropdown(DurationReadout::CHOICES),
        choice!(prefs.interface.duration_readout),
    )
    .info(
        "Whether the time at the right end of the control bar is the media's length or the time \
         left. Clicking the readout switches it too.",
    )
    .keywords(&["remaining time", "duration", "countdown"])
    .advanced(),
];
