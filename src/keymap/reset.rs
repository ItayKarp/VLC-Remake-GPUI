//! "Reset all shortcuts" (S5.6.6) in Settings → Controls: every command gets its default keys
//! back, after a confirmation unless confirmations are off (S5.3.3).

use gpui::{App, PromptLevel, Window};

use super::custom::Shortcuts;
use crate::settings;

/// Asks before giving every command its default keys, or says there's nothing to reset.
pub fn ask_to_reset_all(window: &mut Window, cx: &mut App) {
    if !super::load(cx).is_customized() {
        let detail = "Every shortcut already has its default keys.";
        // Nothing waits for the answer: OK only closes the alert.
        drop(window.prompt(PromptLevel::Info, "Nothing to reset", Some(detail), &["OK"], cx));
        return;
    }
    if !settings::get(cx).prefs.startup.show_dialogs {
        reset_all(cx);
        return;
    }
    let detail = "Every shortcut you changed goes back to its default keys. This can't be undone.";
    let answer = window.prompt(
        PromptLevel::Warning,
        "Reset all keyboard shortcuts?",
        Some(detail),
        &["Reset All", "Cancel"],
        cx,
    );
    cx.spawn(async move |cx| {
        if answer.await == Ok(0) {
            cx.update(reset_all);
        }
    })
    .detach();
}

/// Gives every command its default keys.
fn reset_all(cx: &mut App) {
    super::apply(&Shortcuts::defaults(super::os(cx)), cx);
}
