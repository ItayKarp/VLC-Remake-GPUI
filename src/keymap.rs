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
pub mod commands;
pub mod label;
pub mod menus;
pub mod repeat;
pub mod sections;
#[cfg(test)]
mod tests;

use std::rc::Rc;

use gpui::{
    Action, App, KeyBinding, KeyBindingContextPredicate, Keystroke, OperatingSystem, SharedString,
};

use crate::ui::{focus, modal};
use commands::{Area, COMMANDS, Command};
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

/// Binds the default keymap and keeps labels current across layout switches.
pub fn init(cx: &mut App) {
    let bindings = default_bindings(cx);
    cx.bind_keys(bindings);
    if is_mac(cx) {
        cx.set_menus(menus::mac_menu_bar());
    }
    cx.on_keyboard_layout_change(|cx| {
        tracing::info!("keyboard layout changed to {}", cx.keyboard_layout().name());
        cx.refresh_windows();
    })
    .detach();
}

/// Every command's default bindings for this OS. A command's primary key is
/// added last, so the keymap ranks it first for display.
fn default_bindings(cx: &App) -> Vec<KeyBinding> {
    let mac = is_mac(cx);
    let mut bindings = Vec::new();
    for command in COMMANDS {
        for source in command.keys.for_os(mac).iter().rev() {
            match binding(command, source, cx) {
                Ok(binding) => bindings.push(binding),
                Err(err) => tracing::error!("bad default binding {source:?}: {err:#}"),
            }
        }
    }
    bindings
}

fn binding(command: &Command, source: &str, cx: &App) -> anyhow::Result<KeyBinding> {
    let keystroke = Keystroke::parse_with_os(source, cx.platform_info().os)?;
    let predicate = predicate(command.area, &keystroke)
        .map(|p| KeyBindingContextPredicate::parse(&p).map(Rc::new))
        .transpose()?;
    Ok(KeyBinding::load(source, command.action.boxed_clone(), predicate, false, None, cx)?)
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

/// Every shortcut of `action` as text, the primary one first; empty if it has none.
pub fn shortcuts(action: &dyn Action, cx: &App) -> Vec<SharedString> {
    let mac = is_mac(cx);
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    keymap
        .bindings_for_action(action)
        .rev()
        .filter_map(|binding| binding.keystrokes().first())
        .map(|keystroke| label::text(keystroke.inner(), mac).into())
        .collect()
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
    cx.platform_info().os == OperatingSystem::Mac
}
