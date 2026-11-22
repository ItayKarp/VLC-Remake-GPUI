//! The Settings window's keys (S5.2.5): Ctrl/Cmd+F focuses search, Up and Down move between
//! categories, and Escape clears the search, then closes the window.
//!
//! Up, Down and Escape don't fire while a control that uses them has focus (a dropdown, a
//! slider, a field that puts its value back on Escape). The search field is the exception for
//! Escape: it clears the search from there too.

use std::rc::Rc;

use gpui::{App, KeyBinding, KeyBindingContextPredicate, actions};

use crate::ui::focus;

actions!(
    settings_window,
    [
        /// Focuses the search field and selects what it holds.
        FocusSearch,
        /// Selects the category above the current one.
        PreviousCategory,
        /// Selects the category below the current one.
        NextCategory,
        /// Clears the search, or closes the window when there is none.
        Dismiss,
    ]
);

/// The key context of the window's root.
pub(super) const CONTEXT: &str = "SettingsWindow";
/// The key context around the search field.
pub(super) const SEARCH_CONTEXT: &str = "SettingsSearch";

/// Binds the window's keys.
pub fn bind_keys(cx: &mut App) {
    let free = format!("{CONTEXT} && !{}", focus::CONTROL);
    let free = Some(free.as_str());
    let mut bindings = vec![
        KeyBinding::new("up", PreviousCategory, free),
        KeyBinding::new("down", NextCategory, free),
        KeyBinding::new("escape", Dismiss, free),
        KeyBinding::new("escape", Dismiss, Some(SEARCH_CONTEXT)),
    ];
    // Only `load` reads `secondary-` (Cmd on macOS, Ctrl elsewhere).
    let context = KeyBindingContextPredicate::parse(CONTEXT).map(Rc::new);
    match context.and_then(|context| {
        Ok(KeyBinding::load("secondary-f", Box::new(FocusSearch), Some(context), false, None, cx)?)
    }) {
        Ok(binding) => bindings.push(binding),
        Err(err) => tracing::error!("couldn't bind the Settings search key: {err:#}"),
    }
    cx.bind_keys(bindings);
}
