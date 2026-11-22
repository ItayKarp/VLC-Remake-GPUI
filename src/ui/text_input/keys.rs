//! The text input's actions and their key bindings.
//!
//! Keys are bound in the [`CONTEXT`] key context, so single-letter and Space shortcuts elsewhere
//! don't fire while typing.

use gpui::{App, KeyBinding, actions};

/// The key context every text input sets while it has focus.
pub const CONTEXT: &str = "TextInput";

/// The key context a multi-line text input adds: Enter starts a line, Up and Down move between
/// lines.
pub const MULTI_LINE_CONTEXT: &str = "TextArea";

/// Where Enter submits: a single-line input.
const SINGLE_LINE: &str = "TextInput && !TextArea";

actions!(
    text_input,
    [
        /// Deletes the selection or the grapheme before the caret.
        Backspace,
        /// Deletes the selection or the word before the caret.
        BackspaceWord,
        /// Deletes the selection or the grapheme after the caret.
        Delete,
        /// Deletes the selection or the word after the caret.
        DeleteWord,
        /// Moves the caret back a grapheme.
        Left,
        /// Moves the caret on a grapheme.
        Right,
        /// Moves the caret back a word.
        WordLeft,
        /// Moves the caret on a word.
        WordRight,
        /// Extends the selection back a grapheme.
        SelectLeft,
        /// Extends the selection on a grapheme.
        SelectRight,
        /// Extends the selection back a word.
        SelectWordLeft,
        /// Extends the selection on a word.
        SelectWordRight,
        /// Selects the whole text.
        SelectAll,
        /// Moves the caret to the start.
        Home,
        /// Moves the caret to the end.
        End,
        /// Extends the selection to the start.
        SelectToHome,
        /// Extends the selection to the end.
        SelectToEnd,
        /// Replaces the selection with the clipboard's text.
        Paste,
        /// Moves the selection to the clipboard.
        Cut,
        /// Copies the selection to the clipboard.
        Copy,
        /// Takes back the last edit.
        Undo,
        /// Re-applies the last undone edit.
        Redo,
        /// Enter: tells listeners the value is final.
        Submit,
        /// Enter in a multi-line input: starts a new line.
        Newline,
        /// Moves the caret up a line.
        Up,
        /// Moves the caret down a line.
        Down,
        /// Extends the selection up a line.
        SelectUp,
        /// Extends the selection down a line.
        SelectDown,
        /// Opens the system's emoji and symbol picker.
        ShowCharacterPalette,
    ]
);

/// Picks the macOS or the Windows/Linux spelling of a binding.
macro_rules! platform {
    ($mac:literal, $other:literal) => {
        if cfg!(target_os = "macos") { $mac } else { $other }
    };
}

/// Binds the text input's keys; call once at startup.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys(common_bindings());
    cx.bind_keys(platform_only_bindings());
    cx.bind_keys(multi_line_bindings());
}

/// Bindings only a multi-line input has.
fn multi_line_bindings() -> Vec<KeyBinding> {
    let ctx = Some(MULTI_LINE_CONTEXT);
    vec![
        KeyBinding::new("enter", Newline, ctx),
        KeyBinding::new("shift-enter", Newline, ctx),
        KeyBinding::new(platform!("cmd-enter", "ctrl-enter"), Submit, ctx),
        KeyBinding::new("up", Up, ctx),
        KeyBinding::new("down", Down, ctx),
        KeyBinding::new("shift-up", SelectUp, ctx),
        KeyBinding::new("shift-down", SelectDown, ctx),
    ]
}

/// Bindings every platform has, in its own spelling.
fn common_bindings() -> Vec<KeyBinding> {
    let ctx = Some(CONTEXT);
    // Word movement is Alt on macOS and Ctrl elsewhere; the "command" modifier is
    // Cmd on macOS and Ctrl elsewhere.
    vec![
        KeyBinding::new("backspace", Backspace, ctx),
        KeyBinding::new("delete", Delete, ctx),
        KeyBinding::new("left", Left, ctx),
        KeyBinding::new("right", Right, ctx),
        KeyBinding::new("shift-left", SelectLeft, ctx),
        KeyBinding::new("shift-right", SelectRight, ctx),
        KeyBinding::new("home", Home, ctx),
        KeyBinding::new("end", End, ctx),
        KeyBinding::new("shift-home", SelectToHome, ctx),
        KeyBinding::new("shift-end", SelectToEnd, ctx),
        KeyBinding::new("enter", Submit, Some(SINGLE_LINE)),
        KeyBinding::new(platform!("alt-backspace", "ctrl-backspace"), BackspaceWord, ctx),
        KeyBinding::new(platform!("alt-delete", "ctrl-delete"), DeleteWord, ctx),
        KeyBinding::new(platform!("alt-left", "ctrl-left"), WordLeft, ctx),
        KeyBinding::new(platform!("alt-right", "ctrl-right"), WordRight, ctx),
        KeyBinding::new(platform!("alt-shift-left", "ctrl-shift-left"), SelectWordLeft, ctx),
        KeyBinding::new(platform!("alt-shift-right", "ctrl-shift-right"), SelectWordRight, ctx),
        KeyBinding::new(platform!("cmd-a", "ctrl-a"), SelectAll, ctx),
        KeyBinding::new(platform!("cmd-v", "ctrl-v"), Paste, ctx),
        KeyBinding::new(platform!("cmd-c", "ctrl-c"), Copy, ctx),
        KeyBinding::new(platform!("cmd-x", "ctrl-x"), Cut, ctx),
        KeyBinding::new(platform!("cmd-z", "ctrl-z"), Undo, ctx),
        KeyBinding::new(platform!("cmd-shift-z", "ctrl-shift-z"), Redo, ctx),
        KeyBinding::new("shift-insert", Paste, ctx),
        KeyBinding::new("ctrl-insert", Copy, ctx),
        KeyBinding::new("shift-delete", Cut, ctx),
    ]
}

/// Bindings only one platform's conventions have.
fn platform_only_bindings() -> Vec<KeyBinding> {
    let ctx = Some(CONTEXT);
    if cfg!(target_os = "macos") {
        vec![
            KeyBinding::new("cmd-left", Home, ctx),
            KeyBinding::new("cmd-right", End, ctx),
            KeyBinding::new("cmd-shift-left", SelectToHome, ctx),
            KeyBinding::new("cmd-shift-right", SelectToEnd, ctx),
            KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, ctx),
        ]
    } else {
        vec![KeyBinding::new("ctrl-y", Redo, ctx)]
    }
}
