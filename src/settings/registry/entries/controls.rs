//! Controls ("Mouse, Keyboard & Remote Controls"): the Keyboard Shortcuts dialog and resetting
//! it, skip amounts, the mouse over the video, and the web remote (S8.15).

use super::super::def::{Condition, Control, SettingDef};
use super::super::value::{Binding, Choice, SettingChoice, choice, number, toggle};
use crate::keymap::reset::ask_to_reset_all;
use crate::main_window::{edit_keyboard_shortcuts, web_remote_hint};
use crate::privacy::keychain::Secret;
use crate::settings::registry::Category;
use crate::settings::{ClickAction, DoubleClickAction, HorizontalWheelAction, WheelAction};

const SHORTCUTS: (Category, &str) = (Category::Controls, tr_noop!("Keyboard Shortcuts"));
const SKIPS: (Category, &str) = (Category::Controls, tr_noop!("Skip Amounts"));
const MOUSE: (Category, &str) = (Category::Controls, tr_noop!("Mouse"));
const REMOTES: (Category, &str) = (Category::Controls, tr_noop!("Remotes"));
/// The key of "Web remote control", which its port and password depend on.
const WEB_REMOTE_KEY: &str = "controls.web_remote.enabled";
/// The lowest port the web remote takes: those below need administrator rights on Linux.
const MIN_PORT: f64 = 1024.;
/// The highest port there is.
const MAX_PORT: f64 = 65535.;
/// A skip field's unit.
const SECONDS: Control = Control::Number { unit: tr_noop!("s") };
/// The longest skip: an hour.
const MAX_SKIP: f64 = 3600.;

impl SettingChoice for ClickAction {
    const VALUES: &'static [Self] = &[Self::PlayPause, Self::Nothing];
    const CHOICES: &'static [Choice] = &[
        Choice::new("play_pause", tr_noop!("Play / Pause")),
        Choice::new("nothing", tr_noop!("Nothing")),
    ];
}

impl SettingChoice for DoubleClickAction {
    const VALUES: &'static [Self] = &[Self::ToggleFullscreen, Self::Nothing];
    const CHOICES: &'static [Choice] = &[
        Choice::new("toggle_fullscreen", tr_noop!("Toggle fullscreen")),
        Choice::new("nothing", tr_noop!("Nothing")),
    ];
}

impl SettingChoice for WheelAction {
    const VALUES: &'static [Self] = &[Self::Volume, Self::Nothing];
    const CHOICES: &'static [Choice] =
        &[Choice::new("volume", tr_noop!("Volume")), Choice::new("nothing", tr_noop!("Nothing"))];
}

impl SettingChoice for HorizontalWheelAction {
    const VALUES: &'static [Self] = &[Self::Seek, Self::Nothing];
    const CHOICES: &'static [Choice] =
        &[Choice::new("seek", tr_noop!("Seek")), Choice::new("nothing", tr_noop!("Nothing"))];
}

/// The Controls page's rows.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    SettingDef::new(
        "edit_keyboard_shortcuts",
        SHORTCUTS,
        tr_noop!("Defaults"),
        Control::Button {
            label: tr_noop!("Edit Keyboard Shortcuts"),
            action: edit_keyboard_shortcuts,
        },
        Binding::ACTION,
    )
    .info(tr_noop!("Opens the list of shortcuts in the main window, where a pencil changes a key."))
    .keywords(&["hotkeys", "keys", "keyboard", "bindings", "keymap"]),
    SettingDef::new(
        "reset_keyboard_shortcuts",
        SHORTCUTS,
        tr_noop!("Reset all shortcuts"),
        Control::Button { label: tr_noop!("Reset"), action: ask_to_reset_all },
        Binding::ACTION,
    )
    .info(tr_noop!("Gives every command its default keys back, after asking."))
    .keywords(&["hotkeys", "keys", "keyboard", "bindings", "defaults", "restore"]),
    SettingDef::new(
        "controls.skip.very_short",
        SKIPS,
        tr_noop!("Very short skip"),
        SECONDS,
        number!(u32: prefs.controls.skip.very_short),
    )
    .info(tr_noop!(
        "How far Shift with the Left or Right arrow jumps, and each sideways scroll step."
    ))
    .range(1., MAX_SKIP, 1.)
    .keywords(&["seek", "jump", "shift arrow"]),
    SettingDef::new(
        "controls.skip.short",
        SKIPS,
        tr_noop!("Short skip"),
        SECONDS,
        number!(u32: prefs.controls.skip.short),
    )
    .info(tr_noop!("How far the Left or Right arrow alone jumps."))
    .range(1., MAX_SKIP, 1.)
    .keywords(&["seek", "jump", "arrow keys"]),
    SettingDef::new(
        "controls.skip.medium",
        SKIPS,
        tr_noop!("Medium skip"),
        SECONDS,
        number!(u32: prefs.controls.skip.medium),
    )
    .info(tr_noop!("How far Alt (Option on a Mac) with the Left or Right arrow jumps."))
    .range(1., MAX_SKIP, 1.)
    .keywords(&["seek", "jump", "alt arrow", "option arrow"]),
    SettingDef::new(
        "controls.skip.long",
        SKIPS,
        tr_noop!("Long skip"),
        SECONDS,
        number!(u32: prefs.controls.skip.long),
    )
    .info(tr_noop!("How far Ctrl (Cmd on a Mac) with the Left or Right arrow jumps."))
    .range(1., MAX_SKIP, 1.)
    .keywords(&["seek", "jump", "ctrl arrow", "cmd arrow"]),
    SettingDef::new(
        "controls.click",
        MOUSE,
        tr_noop!("Single click on the video"),
        Control::Dropdown(ClickAction::CHOICES),
        choice!(prefs.controls.click),
    )
    .info(tr_noop!("What clicking the video once does."))
    .keywords(&["mouse", "pause"]),
    SettingDef::new(
        "controls.double_click",
        MOUSE,
        tr_noop!("Double click on the video"),
        Control::Dropdown(DoubleClickAction::CHOICES),
        choice!(prefs.controls.double_click),
    )
    .info(tr_noop!("What double-clicking the video does."))
    .keywords(&["mouse", "fullscreen"]),
    SettingDef::new(
        "controls.wheel",
        MOUSE,
        tr_noop!("Scroll wheel"),
        Control::Dropdown(WheelAction::CHOICES),
        choice!(prefs.controls.wheel),
    )
    .info(tr_noop!(
        "What scrolling up and down over the video does. Volume moves by the Volume step."
    ))
    .keywords(&["mouse", "volume"]),
    SettingDef::new(
        "controls.horizontal_wheel",
        MOUSE,
        tr_noop!("Horizontal scroll"),
        Control::Dropdown(HorizontalWheelAction::CHOICES),
        choice!(prefs.controls.horizontal_wheel),
    )
    .info(tr_noop!(
        "What scrolling sideways over the video does, with a trackpad or a tilting wheel. Seek \
         moves by the very short skip."
    ))
    .keywords(&["mouse", "trackpad", "seek"]),
    SettingDef::new(
        "controls.reverse_scroll",
        MOUSE,
        tr_noop!("Reverse scroll direction"),
        Control::Toggle,
        toggle!(prefs.controls.reverse_scroll),
    )
    .info(tr_noop!(
        "Scrolling over the video turns the other way: down is louder and left seeks forward. The \
         volume slider keeps its direction."
    ))
    .keywords(&["mouse", "wheel", "trackpad", "natural", "invert"]),
    SettingDef::new(
        WEB_REMOTE_KEY,
        REMOTES,
        tr_noop!("Web remote control"),
        Control::Toggle,
        toggle!(prefs.controls.web_remote.enabled),
    )
    .info(tr_noop!(
        "Lets a phone on this network play, pause, change the volume and pick from the queue in \
         its browser, at the address shown here. It signs in with the password below; nothing \
         outside your local network is answered."
    ))
    .hint(web_remote_hint)
    .keywords(&["remote", "phone", "web interface", "http", "browser", "lan"]),
    SettingDef::new(
        "controls.web_remote.port",
        REMOTES,
        tr_noop!("Port"),
        Control::Number { unit: "" },
        number!(u16: prefs.controls.web_remote.port),
    )
    .info(tr_noop!(
        "The port the web remote listens on. Pick another if a program already uses it."
    ))
    .range(MIN_PORT, MAX_PORT, 1.)
    .depends_on(WEB_REMOTE_KEY, Condition::IsOn)
    .keywords(&["remote", "web interface", "http", "network"]),
    SettingDef::new(
        "web_remote_password",
        REMOTES,
        tr_noop!("Password"),
        Control::Secret(Secret::WebRemotePassword),
        Binding::ACTION,
    )
    .info(tr_noop!(
        "Phones need it to sign in; without one, nobody can. Kept in your system's keychain, never \
         in VLC's settings file. Press Enter to save it."
    ))
    .depends_on(WEB_REMOTE_KEY, Condition::IsOn)
    .keywords(&["remote", "web interface", "login", "keychain", "pairing"]),
];
