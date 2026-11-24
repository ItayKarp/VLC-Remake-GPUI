//! "Reset all shortcuts" (S5.6.6) in Settings → Controls: every command gets its default keys
//! back, after a confirmation unless confirmations are off (S5.3.3).

use gpui::{App, PromptLevel, Window};

use super::custom::Shortcuts;
use crate::settings;

/// Asks before giving every command its default keys, or says there's nothing to reset.
pub fn ask_to_reset_all(window: &mut Window, cx: &mut App) {
    if !super::load(cx).is_customized() {
        let detail = tr!("Every shortcut already has its default keys.");
        let title = tr!("Nothing to reset");
        // Nothing waits for the answer: OK only closes the alert.
        drop(window.prompt(PromptLevel::Info, title, Some(detail), &[tr!("OK")], cx));
        return;
    }
    if !settings::get(cx).prefs.startup.show_dialogs {
        reset_all(cx);
        return;
    }
    let detail =
        tr!("Every shortcut you changed goes back to its default keys. This can't be undone.");
    let answer = window.prompt(
        PromptLevel::Warning,
        tr!("Reset all keyboard shortcuts?"),
        Some(detail),
        &[tr!("Reset All"), tr!("Cancel")],
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
