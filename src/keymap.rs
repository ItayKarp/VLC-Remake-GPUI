//! Commands, the default keymap, and shortcut labels (S0.5).
//!
//! Every command is a GPUI action ([`actions`]) with one row in the command table
//! ([`commands::COMMANDS`]): label, group, where it works, default keys, and what a
//! held key does. The table is the single source of truth: [`init`] binds its keys,
//! and onboarding, tooltips and menus ask the keymap for a command's shortcut
//! ([`shortcut`], [`key_caps`]) instead of spelling it themselves.
//!
//! **Key contexts (S0.5.2).** Each binding gets a context predicate from its
//! command's [`Area`] and its key ([`predicate`]):
//! - shortcuts with Ctrl, Alt, Cmd or an F-key work anywhere in their area;
//! - Space, Enter, Escape, the arrows, Home and End (with or without modifiers) don't
//!   fire while a kit control has focus ([`focus::CONTROL`]): a button presses on
//!   Space, a slider moves on arrows, a text field moves its caret on Alt+Left;
//! - letters and symbols don't fire while a control takes typed text
//!   ([`focus::TYPING`]: text inputs, menus and dropdowns with type-ahead);
//! - nothing but global shortcuts fires while a modal is open.
//!
//! **Keyboard layouts (S0.5.5).** Bindings name the *character*, not the physical
//! key. GPUI matches a binding against the typed character as well as the key, so
//! `<` works whether it is Shift+`,` (US, UK) or a key of its own (German ISO,
//! AZERTY), and P/N need no Shift on any layout. On non-Latin layouts (Hebrew,
//! Russian) GPUI reports letters by their Latin virtual key, so F, L, Q, V and B
//! still work. A layout switch therefore needs no rebinding; windows re-render so
//! shortcut labels stay current.

pub mod actions;
pub mod check;
pub mod commands;
pub mod custom;
pub mod label;
pub mod menus;
pub mod repeat;
pub mod reset;
pub mod sections;
#[cfg(test)]
mod tests;

use std::rc::Rc;

use gpui::{
    Action, App, KeyBinding, KeyBindingContextPredicate, Keystroke, OperatingSystem, SharedString,
};

use crate::settings;
use crate::ui::{focus, modal};
use commands::{Area, Command};
use custom::Shortcuts;
use label::KeyCap;

/// Key context identifiers set by windows and views.
pub mod contexts {
    /// The main window's root.
    pub const MAIN_WINDOW: &str = "MainWindow";
    /// The player (the main window until Stage 2 gives it a view of its own).
    pub const PLAYER: &str = "Player";
    /// The library (Stage 4).
    pub const LIBRARY: &str = "Library";
}

/// Keys that kit controls use themselves (press, move, close).
const NAVIGATION_KEYS: &[&str] = &[
    "space",
    "enter",
    "escape",
    "left",
    "right",
    "up",
    "down",
    "home",
    "end",
    "pageup",
    "pagedown",
    "tab",
    "backspace",
    "delete",
];

/// Binds the keymap (the defaults with the user's changes) and keeps labels current across
/// layout switches.
pub fn init(cx: &mut App) {
    let shortcuts = load(cx);
    cx.bind_keys(command_bindings(&shortcuts, cx));
    if is_mac(cx) {
        cx.set_menus(menus::mac_menu_bar());
    }
    cx.on_keyboard_layout_change(|cx| {
        tracing::info!("keyboard layout changed to {}", cx.keyboard_layout().name());
        cx.refresh_windows();
    })
    .detach();
}

/// Saves `shortcuts` as the user's keymap and binds them in place of the commands' current
/// keys. Other bindings (text fields, the Settings window) keep their place, and so their
/// precedence.
pub fn apply(shortcuts: &Shortcuts, cx: &mut App) {
    let keymap = shortcuts.to_settings();
    settings::update(cx, |settings| settings.prefs.keymap = keymap);
    let fresh = command_bindings(shortcuts, cx);
    let old: Vec<KeyBinding> = cx.key_bindings().borrow().bindings().cloned().collect();
    let is_command = |binding: &KeyBinding| commands::find(binding.action()).is_some();
    let at = old.iter().position(is_command).unwrap_or(old.len());
    let (before, after) = old.split_at(at);
    let others = after.iter().filter(|binding| !is_command(binding)).cloned();
    let bindings: Vec<KeyBinding> = before.iter().cloned().chain(fresh).chain(others).collect();
    cx.clear_key_bindings();
    cx.bind_keys(bindings);
    if is_mac(cx) {
        cx.set_menus(menus::mac_menu_bar());
    }
}

/// The user's shortcuts as the settings have them, reporting what had to be left out.
pub fn load(cx: &App) -> Shortcuts {
    let (shortcuts, problems) = Shortcuts::load(&settings::get(cx).prefs.keymap, os(cx));
    for problem in problems {
        tracing::warn!("keymap: left out {problem}");
    }
    shortcuts
}

/// Every command's bindings. A command's primary key is added last, so the keymap ranks it
/// first for display.
fn command_bindings(shortcuts: &Shortcuts, cx: &App) -> Vec<KeyBinding> {
    let mut bindings = Vec::new();
    for (command, keys) in shortcuts.iter() {
        for keystroke in keys.iter().rev() {
            match binding(command, keystroke, cx) {
                Ok(binding) => bindings.push(binding),
                Err(err) => tracing::error!("bad binding {keystroke:?}: {err:#}"),
            }
        }
    }
    bindings
}

fn binding(command: &Command, keystroke: &Keystroke, cx: &App) -> anyhow::Result<KeyBinding> {
    let source = keystroke.unparse(os(cx));
    let predicate = predicate(command.area, keystroke)
        .map(|p| KeyBindingContextPredicate::parse(&p).map(Rc::new))
        .transpose()?;
    Ok(KeyBinding::load(&source, command.action.boxed_clone(), predicate, false, None, cx)?)
}

/// The context predicate a binding of `keystroke` in `area` gets (see the module
/// docs); `None` for global bindings.
pub fn predicate(area: Area, keystroke: &Keystroke) -> Option<String> {
    let base = match area {
        Area::Global => return None,
        Area::App => contexts::MAIN_WINDOW,
        Area::Player => contexts::PLAYER,
        Area::Library => contexts::LIBRARY,
    };
    let modifiers = &keystroke.modifiers;
    let chord = modifiers.control || modifiers.alt || modifiers.platform || modifiers.function;
    let function_key = keystroke.key.len() > 1 && keystroke.key.starts_with('f');
    // A control uses its navigation keys with modifiers too (Alt+Left moves a text caret by a
    // word), so those never take a chord's pass.
    let guard = if NAVIGATION_KEYS.contains(&keystroke.key.as_str()) {
        format!(" && !{}", focus::CONTROL)
    } else if chord || function_key {
        String::new()
    } else {
        format!(" && !{}", focus::TYPING)
    };
    Some(format!("{base} && !{}{guard}", modal::CONTEXT))
}

/// The primary shortcut of `action` as text ("Ctrl + O", "⌘O"), if it has one.
pub fn shortcut(action: &dyn Action, cx: &App) -> Option<SharedString> {
    primary_keystroke(action, cx).map(|keystroke| label::text(&keystroke, is_mac(cx)).into())
}

/// The primary shortcut of `action` as key caps; empty if it has none.
pub fn key_caps(action: &dyn Action, cx: &App) -> Vec<KeyCap> {
    primary_keystroke(action, cx).map_or_else(Vec::new, |k| label::caps(&k, is_mac(cx)))
}

/// The keystroke of the binding the keymap ranks first for `action`.
fn primary_keystroke(action: &dyn Action, cx: &App) -> Option<Keystroke> {
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    let binding = keymap.bindings_for_action(action).last()?;
    binding.keystrokes().first().map(|keystroke| keystroke.inner().clone())
}

/// Whether shortcuts follow macOS conventions (Cmd, symbols).
fn is_mac(cx: &App) -> bool {
    os(cx) == OperatingSystem::Mac
}

/// The OS whose key names and conventions shortcuts use.
pub fn os(cx: &App) -> OperatingSystem {
    cx.platform_info().os
}
