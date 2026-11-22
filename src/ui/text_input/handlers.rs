//! What the text input does for each action and mouse event.

use std::time::Instant;

use gpui::{
    ClipboardItem, Context, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Window,
};

use super::keys::{
    Backspace, BackspaceWord, Copy, Cut, Delete, DeleteWord, End, Home, Left, Paste, Redo, Right,
    SelectAll, SelectLeft, SelectRight, SelectToEnd, SelectToHome, SelectWordLeft, SelectWordRight,
    ShowCharacterPalette, Submit, Undo, WordLeft, WordRight,
};
use super::{Event, TextInput};

impl TextInput {
    /// Deletes the selection or the grapheme before the caret.
    pub(super) fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        let changed = self.state.backspace(false, Instant::now());
        self.edited(changed, window, cx);
    }

    /// Deletes the selection or the word before the caret.
    pub(super) fn backspace_word(
        &mut self,
        _: &BackspaceWord,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let changed = self.state.backspace(true, Instant::now());
        self.edited(changed, window, cx);
    }

    /// Deletes the selection or the grapheme after the caret.
    pub(super) fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        let changed = self.state.delete(false, Instant::now());
        self.edited(changed, window, cx);
    }

    /// Deletes the selection or the word after the caret.
    pub(super) fn delete_word(
        &mut self,
        _: &DeleteWord,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let changed = self.state.delete(true, Instant::now());
        self.edited(changed, window, cx);
    }

    /// Moves the caret back a grapheme.
    pub(super) fn left(&mut self, _: &Left, window: &mut Window, cx: &mut Context<Self>) {
        self.state.left(false);
        self.moved(window, cx);
    }

    /// Moves the caret on a grapheme.
    pub(super) fn right(&mut self, _: &Right, window: &mut Window, cx: &mut Context<Self>) {
        self.state.right(false);
        self.moved(window, cx);
    }

    /// Moves the caret back a word.
    pub(super) fn word_left(&mut self, _: &WordLeft, window: &mut Window, cx: &mut Context<Self>) {
        self.state.left(true);
        self.moved(window, cx);
    }

    /// Moves the caret on a word.
    pub(super) fn word_right(
        &mut self,
        _: &WordRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.right(true);
        self.moved(window, cx);
    }

    /// Extends the selection back a grapheme.
    pub(super) fn select_left(
        &mut self,
        _: &SelectLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.select_left(false);
        self.moved(window, cx);
    }

    /// Extends the selection on a grapheme.
    pub(super) fn select_right(
        &mut self,
        _: &SelectRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.select_right(false);
        self.moved(window, cx);
    }

    /// Extends the selection back a word.
    pub(super) fn select_word_left(
        &mut self,
        _: &SelectWordLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.select_left(true);
        self.moved(window, cx);
    }

    /// Extends the selection on a word.
    pub(super) fn select_word_right(
        &mut self,
        _: &SelectWordRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.select_right(true);
        self.moved(window, cx);
    }

    /// Selects the whole text.
    pub(super) fn select_all(
        &mut self,
        _: &SelectAll,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.select_all();
        self.moved(window, cx);
    }

    /// Moves the caret to the start of its line.
    pub(super) fn home(&mut self, _: &Home, window: &mut Window, cx: &mut Context<Self>) {
        self.state.move_to(self.state.line_start(self.state.cursor()));
        self.moved(window, cx);
    }

    /// Moves the caret to the end of its line.
    pub(super) fn end(&mut self, _: &End, window: &mut Window, cx: &mut Context<Self>) {
        self.state.move_to(self.state.line_end(self.state.cursor()));
        self.moved(window, cx);
    }

    /// Extends the selection to the start of the caret's line.
    pub(super) fn select_to_home(
        &mut self,
        _: &SelectToHome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.select_to(self.state.line_start(self.state.cursor()));
        self.moved(window, cx);
    }

    /// Extends the selection to the end of the caret's line.
    pub(super) fn select_to_end(
        &mut self,
        _: &SelectToEnd,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.select_to(self.state.line_end(self.state.cursor()));
        self.moved(window, cx);
    }

    /// Replaces the selection with the clipboard's text.
    pub(super) fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let changed = self.state.replace(None, &text, Instant::now());
            self.edited(changed, window, cx);
        }
    }

    /// Copies the selection to the clipboard.
    pub(super) fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        let selected = self.state.selected_text();
        if !selected.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(selected.to_owned()));
        }
    }

    /// Moves the selection to the clipboard.
    pub(super) fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        let selected = self.state.selected_text();
        if !selected.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(selected.to_owned()));
            let changed = self.state.replace(None, "", Instant::now());
            self.edited(changed, window, cx);
        }
    }

    /// Takes back the last edit.
    pub(super) fn undo(&mut self, _: &Undo, window: &mut Window, cx: &mut Context<Self>) {
        let changed = self.state.undo();
        self.edited(changed, window, cx);
    }

    /// Re-applies the last undone edit.
    pub(super) fn redo(&mut self, _: &Redo, window: &mut Window, cx: &mut Context<Self>) {
        let changed = self.state.redo();
        self.edited(changed, window, cx);
    }

    /// Emits [`Event::Submitted`]. An associated function rather than a method because it
    /// reads nothing from the input; it still has a listener's shape.
    pub(super) fn submit(_: &mut Self, _: &Submit, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(Event::Submitted);
        // Let a dialog's Enter handling see it too.
        cx.propagate();
    }

    /// Opens the system's emoji and symbol picker. An associated function for the same reason
    /// as [`Self::submit`].
    pub(super) fn show_character_palette(
        _: &mut Self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    /// The text offset under a window position, from the last painted layout.
    pub(super) fn index_for_position(&self, position: Point<Pixels>) -> usize {
        let (Some(bounds), Some(lines)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if self.state.text().is_empty() {
            return 0;
        }
        lines.index_for(position - bounds.origin + self.scroll)
    }

    /// Places the caret, or selects a word (double-click) or everything (triple-click).
    pub(super) fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        let index = self.index_for_position(event.position);
        match event.click_count {
            2 => {
                let word = self.state.word_at(index);
                self.state.select_range(word);
            }
            n if n >= 3 => self.state.select_all(),
            _ => {
                self.selecting = true;
                if event.modifiers.shift {
                    self.state.select_to(index);
                } else {
                    self.state.move_to(index);
                }
            }
        }
        self.moved(window, cx);
    }

    /// Ends a drag selection.
    pub(super) fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
    }

    /// Extends a drag selection to the pointer.
    pub(super) fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selecting {
            self.state.select_to(self.index_for_position(event.position));
            self.moved(window, cx);
        }
    }
}
