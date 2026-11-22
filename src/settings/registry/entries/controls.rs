//! Controls ("Mouse, Keyboard & Remote Controls"): the Keyboard Shortcuts dialog and resetting
//! it, skip amounts, and the mouse over the video. Remotes come with S8.15.

use super::super::def::{Control, SettingDef};
use super::super::value::{Binding, Choice, SettingChoice, choice, number, toggle};
use crate::keymap::reset::ask_to_reset_all;
use crate::main_window::edit_keyboard_shortcuts;
use crate::settings::registry::Category;
use crate::settings::{ClickAction, DoubleClickAction, HorizontalWheelAction, WheelAction};

const SHORTCUTS: (Category, &str) = (Category::Controls, "Keyboard Shortcuts");
const SKIPS: (Category, &str) = (Category::Controls, "Skip Amounts");
const MOUSE: (Category, &str) = (Category::Controls, "Mouse");
/// A skip field's unit.
const SECONDS: Control = Control::Number { unit: "s" };
/// The longest skip: an hour.
const MAX_SKIP: f64 = 3600.;

impl SettingChoice for ClickAction {
    const VALUES: &'static [Self] = &[Self::PlayPause, Self::Nothing];
    const CHOICES: &'static [Choice] =
        &[Choice::new("play_pause", "Play / Pause"), Choice::new("nothing", "Nothing")];
}

impl SettingChoice for DoubleClickAction {
    const VALUES: &'static [Self] = &[Self::ToggleFullscreen, Self::Nothing];
    const CHOICES: &'static [Choice] =
        &[Choice::new("toggle_fullscreen", "Toggle fullscreen"), Choice::new("nothing", "Nothing")];
}

impl SettingChoice for WheelAction {
    const VALUES: &'static [Self] = &[Self::Volume, Self::Nothing];
    const CHOICES: &'static [Choice] =
        &[Choice::new("volume", "Volume"), Choice::new("nothing", "Nothing")];
}

impl SettingChoice for HorizontalWheelAction {
    const VALUES: &'static [Self] = &[Self::Seek, Self::Nothing];
    const CHOICES: &'static [Choice] =
        &[Choice::new("seek", "Seek"), Choice::new("nothing", "Nothing")];
}

/// The Controls page's rows.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    SettingDef::new(
        "edit_keyboard_shortcuts",
        SHORTCUTS,
        "Defaults",
        Control::Button { label: "Edit Keyboard Shortcuts", action: edit_keyboard_shortcuts },
        Binding::ACTION,
    )
    .info("Opens the list of shortcuts in the main window, where a pencil changes a key.")
    .keywords(&["hotkeys", "keys", "keyboard", "bindings", "keymap"]),
    SettingDef::new(
        "reset_keyboard_shortcuts",
        SHORTCUTS,
        "Reset all shortcuts",
        Control::Button { label: "Reset", action: ask_to_reset_all },
        Binding::ACTION,
    )
    .info("Gives every command its default keys back, after asking.")
    .keywords(&["hotkeys", "keys", "keyboard", "bindings", "defaults", "restore"]),
    SettingDef::new(
        "controls.skip.very_short",
        SKIPS,
        "Very short skip",
        SECONDS,
        number!(u32: prefs.controls.skip.very_short),
    )
    .info("How far Shift with the Left or Right arrow jumps, and each sideways scroll step.")
    .range(1., MAX_SKIP, 1.)
    .keywords(&["seek", "jump", "shift arrow"]),
    SettingDef::new(
        "controls.skip.short",
        SKIPS,
        "Short skip",
        SECONDS,
        number!(u32: prefs.controls.skip.short),
    )
    .info("How far the Left or Right arrow alone jumps.")
    .range(1., MAX_SKIP, 1.)
    .keywords(&["seek", "jump", "arrow keys"]),
    SettingDef::new(
        "controls.skip.medium",
        SKIPS,
        "Medium skip",
        SECONDS,
        number!(u32: prefs.controls.skip.medium),
    )
    .info("How far Alt (Option on a Mac) with the Left or Right arrow jumps.")
    .range(1., MAX_SKIP, 1.)
    .keywords(&["seek", "jump", "alt arrow", "option arrow"]),
    SettingDef::new(
        "controls.skip.long",
        SKIPS,
        "Long skip",
        SECONDS,
        number!(u32: prefs.controls.skip.long),
    )
    .info("How far Ctrl (Cmd on a Mac) with the Left or Right arrow jumps.")
    .range(1., MAX_SKIP, 1.)
    .keywords(&["seek", "jump", "ctrl arrow", "cmd arrow"]),
    SettingDef::new(
        "controls.click",
        MOUSE,
        "Single click on the video",
        Control::Dropdown(ClickAction::CHOICES),
        choice!(prefs.controls.click),
    )
    .info("What clicking the video once does.")
    .keywords(&["mouse", "pause"]),
    SettingDef::new(
        "controls.double_click",
        MOUSE,
        "Double click on the video",
        Control::Dropdown(DoubleClickAction::CHOICES),
        choice!(prefs.controls.double_click),
    )
    .info("What double-clicking the video does.")
    .keywords(&["mouse", "fullscreen"]),
    SettingDef::new(
        "controls.wheel",
        MOUSE,
        "Scroll wheel",
        Control::Dropdown(WheelAction::CHOICES),
        choice!(prefs.controls.wheel),
    )
    .info("What scrolling up and down over the video does. Volume moves by the Volume step.")
    .keywords(&["mouse", "volume"]),
    SettingDef::new(
        "controls.horizontal_wheel",
        MOUSE,
        "Horizontal scroll",
        Control::Dropdown(HorizontalWheelAction::CHOICES),
        choice!(prefs.controls.horizontal_wheel),
    )
    .info(
        "What scrolling sideways over the video does, with a trackpad or a tilting wheel. Seek \
         moves by the very short skip.",
    )
    .keywords(&["mouse", "trackpad", "seek"]),
    SettingDef::new(
        "controls.reverse_scroll",
        MOUSE,
        "Reverse scroll direction",
        Control::Toggle,
        toggle!(prefs.controls.reverse_scroll),
    )
    .info(
        "Scrolling over the video turns the other way: down is louder and left seeks forward. \
         The volume slider keeps its direction.",
    )
    .keywords(&["mouse", "wheel", "trackpad", "natural", "invert"]),
];
