//! Interface: when the controls hide, the now-playing notification and the system tray (S5.4.2,
//! S5.4.3, S8.16). The Skin and Window sections are in `window.rs`.

use super::super::def::{Condition, Control, Marks, SettingDef};
use super::super::rules::trim;
use super::super::value::{Binding, Choice, SettingChoice, Value, choice, toggle};
use crate::settings::registry::Category;
use crate::settings::{AutoHide, DurationReadout, NowPlayingNotification};

const CONTROLS: (Category, &str) = (Category::Interface, tr_noop!("Playback Controls"));
const NOTIFICATIONS: (Category, &str) =
    (Category::Interface, tr_noop!("Notifications and System Tray"));
/// What the tray is called where VLC runs.
#[cfg(target_os = "macos")]
const SYSTEM_TRAY_LABEL: &str = tr_noop!("Show VLC in the menu bar");
#[cfg(not(target_os = "macos"))]
const SYSTEM_TRAY_LABEL: &str = tr_noop!("Show VLC in the system tray");
#[cfg(target_os = "macos")]
const KEEP_PLAYING_LABEL: &str = tr_noop!("Keep playing in the menu bar when the window closes");
#[cfg(not(target_os = "macos"))]
const KEEP_PLAYING_LABEL: &str = tr_noop!("Keep playing in the tray when the window closes");
/// Milliseconds in the second "Hide after" is shown in.
const MS_PER_SECOND: f64 = 1000.;

impl SettingChoice for AutoHide {
    const VALUES: &'static [Self] = &[Self::InFullscreenOnly, Self::Always, Self::Never];
    const CHOICES: &'static [Choice] = &[
        Choice::new("in_fullscreen_only", tr_noop!("In fullscreen only")),
        Choice::new("always", tr_noop!("Always")),
        Choice::new("never", tr_noop!("Never")),
    ];
}

impl SettingChoice for NowPlayingNotification {
    const VALUES: &'static [Self] = &[Self::Never, Self::InBackground, Self::Always];
    const CHOICES: &'static [Choice] = &[
        Choice::new("never", tr_noop!("Never")),
        Choice::new("in_background", tr_noop!("When VLC is in the background")),
        Choice::new("always", tr_noop!("Always")),
    ];
}

impl SettingChoice for DurationReadout {
    const VALUES: &'static [Self] = &[Self::Total, Self::Remaining];
    const CHOICES: &'static [Choice] = &[
        Choice::new("total", tr_noop!("Total length")),
        Choice::new("remaining", tr_noop!("Time left")),
    ];
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
    tr!("{value} s", value = trim(value))
}

/// The Interface page's rows.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    SettingDef::new(
        "interface.start_in_compact_mode",
        CONTROLS,
        tr_noop!("Start in compact mode"),
        Control::Toggle,
        toggle!(prefs.interface.start_in_compact_mode),
    )
    .info(tr_noop!(
        "Plays the first media you open after starting VLC in the small Picture in Picture window, \
         above other windows, with the main window minimized."
    ))
    .keywords(&["miniplayer", "picture in picture", "pip", "mini player", "compact"]),
    SettingDef::new(
        "interface.auto_hide_controls",
        CONTROLS,
        tr_noop!("Auto-hide the controls"),
        Control::Dropdown(AutoHide::CHOICES),
        choice!(prefs.interface.auto_hide_controls),
    )
    .info(tr_noop!(
        "Where the title and control bars hide while video plays and the pointer rests. They come \
         back when you move the pointer."
    ))
    .keywords(&["hide toolbar", "hide bars", "fullscreen controls"]),
    SettingDef::new(
        "interface.hide_controls_after_ms",
        CONTROLS,
        tr_noop!("Hide after"),
        Control::Slider { marks: Marks::None, format: seconds },
        HIDE_AFTER,
    )
    .info(tr_noop!("How long the pointer rests before the controls hide."))
    .range(0.5, 10., 0.5)
    .depends_on("interface.auto_hide_controls", Condition::IsNot("never"))
    .keywords(&["delay", "timeout"]),
    SettingDef::new(
        "interface.show_controls_in_fullscreen",
        CONTROLS,
        tr_noop!("Show controls in fullscreen"),
        Control::Toggle,
        toggle!(prefs.interface.show_controls_in_fullscreen),
    )
    .info(tr_noop!(
        "Shows the controls when you move the pointer in fullscreen. Off, only the \"Show \
         interface controls in fullscreen\" key (I) brings them up."
    )),
    SettingDef::new(
        "interface.keep_controls_while_paused",
        CONTROLS,
        tr_noop!("Keep the controls while paused"),
        Control::Toggle,
        toggle!(prefs.interface.keep_controls_while_paused),
    )
    .info(tr_noop!("Keeps the title and control bars up while playback is paused."))
    .advanced(),
    SettingDef::new(
        "interface.duration_readout",
        CONTROLS,
        tr_noop!("Time readout shows"),
        Control::Dropdown(DurationReadout::CHOICES),
        choice!(prefs.interface.duration_readout),
    )
    .info(tr_noop!(
        "Whether the time at the right end of the control bar is the media's length or the time \
         left. Clicking the readout switches it too."
    ))
    .keywords(&["remaining time", "duration", "countdown"])
    .advanced(),
    SettingDef::new(
        "interface.subtitle_timeline",
        CONTROLS,
        tr_noop!("Show subtitle timeline on the seek bar"),
        Control::Toggle,
        toggle!(prefs.interface.subtitle_timeline),
    )
    .info(tr_noop!(
        "Turns the seek bar into a close-up of the time around the playhead, with a block for each \
         line of the subtitle showing. The gear button's Playback tab switches it too."
    ))
    .keywords(&["subtitle timeline", "seekbar", "cues", "lines"])
    .advanced(),
    SettingDef::new(
        "interface.now_playing_notification",
        NOTIFICATIONS,
        tr_noop!("Show what starts playing"),
        Control::Dropdown(NowPlayingNotification::CHOICES),
        choice!(prefs.interface.now_playing_notification),
    )
    .info(tr_noop!(
        "Shows a system notification with the title of each item as it starts playing. In the \
         background means only while you are using another app."
    ))
    .keywords(&["notify", "notification", "popup", "track change", "now playing"]),
    SettingDef::new(
        "interface.system_tray",
        NOTIFICATIONS,
        SYSTEM_TRAY_LABEL,
        Control::Toggle,
        toggle!(prefs.interface.system_tray),
    )
    .info(tr_noop!(
        "Shows VLC's icon with a menu to play or pause, skip, show VLC or quit, even while its \
         window is hidden."
    ))
    .danger_hint(crate::tray::unavailable_hint)
    .keywords(&["tray", "menu bar", "status bar", "icon", "minimize", "background"]),
    SettingDef::new(
        "interface.keep_playing_in_tray",
        NOTIFICATIONS,
        KEEP_PLAYING_LABEL,
        Control::Toggle,
        toggle!(prefs.interface.keep_playing_in_tray),
    )
    .info(tr_noop!(
        "Closing the main window hides it and playback goes on. Show VLC in the icon's menu brings \
         the window back; Quit stops playback."
    ))
    .depends_on("interface.system_tray", Condition::IsOn)
    .keywords(&["tray", "minimize to tray", "close", "background", "hide"]),
];
