//! Single-line text input (S0.4.9), built on GPUI's input example: caret,
//! selection (mouse, Shift+arrows, double-click for a word, select all),
//! clipboard, IME composition, undo/redo, a placeholder and validation states.
//!
//! The editing rules live in [`EditState`], which has no GPUI types, so they're
//! unit tested. Offsets are UTF-8 byte offsets on grapheme boundaries; IME calls
//! arrive in UTF-16 and are converted at the edge.
//!
//! Keys are bound in the `TextInput` context, so single-letter and Space
//! shortcuts elsewhere don't fire while typing (S0.5.2 builds on this).

use std::ops::Range;
use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, ElementId, ElementInputHandler, EventEmitter,
    FocusHandle, Focusable, GlobalElementId, KeyBinding, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, ShapedLine, SharedString, Style,
    TextRun, UTF16Selection, UnderlineStyle, Window, accesskit, actions, div, fill, point,
    prelude::*, px, relative, size,
};
use unicode_segmentation::UnicodeSegmentation;

use super::{focus, metrics};
use crate::theme;

pub const CONTEXT: &str = "TextInput";

actions!(
    text_input,
    [
        Backspace,
        BackspaceWord,
        Delete,
        DeleteWord,
        Left,
        Right,
        WordLeft,
        WordRight,
        SelectLeft,
        SelectRight,
        SelectWordLeft,
        SelectWordRight,
        SelectAll,
        Home,
        End,
        SelectToHome,
        SelectToEnd,
        Paste,
        Cut,
        Copy,
        Undo,
        Redo,
        Submit,
        ShowCharacterPalette,
    ]
);

/// Picks the macOS or the Windows/Linux spelling of a binding.
macro_rules! platform {
    ($mac:literal, $other:literal) => {
        if cfg!(target_os = "macos") {
            $mac
        } else {
            $other
        }
    };
}

pub fn bind_keys(cx: &mut App) {
    let ctx = Some(CONTEXT);
    // Word movement is Alt on macOS and Ctrl elsewhere; the "command" modifier is
    // Cmd on macOS and Ctrl elsewhere.
    cx.bind_keys([
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
        KeyBinding::new("enter", Submit, ctx),
        KeyBinding::new(
            platform!("alt-backspace", "ctrl-backspace"),
            BackspaceWord,
            ctx,
        ),
        KeyBinding::new(platform!("alt-delete", "ctrl-delete"), DeleteWord, ctx),
        KeyBinding::new(platform!("alt-left", "ctrl-left"), WordLeft, ctx),
        KeyBinding::new(platform!("alt-right", "ctrl-right"), WordRight, ctx),
        KeyBinding::new(
            platform!("alt-shift-left", "ctrl-shift-left"),
            SelectWordLeft,
            ctx,
        ),
        KeyBinding::new(
            platform!("alt-shift-right", "ctrl-shift-right"),
            SelectWordRight,
            ctx,
        ),
        KeyBinding::new(platform!("cmd-a", "ctrl-a"), SelectAll, ctx),
        KeyBinding::new(platform!("cmd-v", "ctrl-v"), Paste, ctx),
        KeyBinding::new(platform!("cmd-c", "ctrl-c"), Copy, ctx),
        KeyBinding::new(platform!("cmd-x", "ctrl-x"), Cut, ctx),
        KeyBinding::new(platform!("cmd-z", "ctrl-z"), Undo, ctx),
        KeyBinding::new(platform!("cmd-shift-z", "ctrl-shift-z"), Redo, ctx),
        KeyBinding::new("shift-insert", Paste, ctx),
        KeyBinding::new("ctrl-insert", Copy, ctx),
        KeyBinding::new("shift-delete", Cut, ctx),
    ]);
    if cfg!(target_os = "macos") {
        cx.bind_keys([
            KeyBinding::new("cmd-left", Home, ctx),
            KeyBinding::new("cmd-right", End, ctx),
            KeyBinding::new("cmd-shift-left", SelectToHome, ctx),
            KeyBinding::new("cmd-shift-right", SelectToEnd, ctx),
            KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, ctx),
        ]);
    } else {
        cx.bind_keys([KeyBinding::new("ctrl-y", Redo, ctx)]);
    }
}

// ---------------------------------------------------------------------------
// Editing rules
// ---------------------------------------------------------------------------

/// Edits typed within this long of each other undo together.
const UNDO_COALESCE: Duration = Duration::from_millis(800);
const UNDO_LIMIT: usize = 100;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    text: String,
    selection: Range<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditKind {
    Typing,
    Deleting,
    Other,
}

/// Text, selection and undo history of a single-line input.
#[derive(Clone, Debug, Default)]
pub struct EditState {
    text: String,
    /// Always on grapheme boundaries; `start <= end`.
    selection: Range<usize>,
    /// The caret is at `selection.start` instead of `end`.
    reversed: bool,
    /// IME composition in progress.
    marked: Option<Range<usize>>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    last_edit: Option<(EditKind, Instant)>,
    max_len: Option<usize>,
}

impl EditState {
    pub fn new(text: &str) -> Self {
        let text = single_line(text);
        let end = text.len();
        Self {
            text,
            selection: end..end,
            ..Default::default()
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    #[cfg(test)]
    pub fn selection(&self) -> Range<usize> {
        self.selection.clone()
    }

    pub fn cursor(&self) -> usize {
        if self.reversed {
            self.selection.start
        } else {
            self.selection.end
        }
    }

    pub fn selected_text(&self) -> &str {
        &self.text[self.selection.clone()]
    }

    /// Replaces the whole text (from outside, not the user); clears history.
    pub fn set_text(&mut self, text: &str) {
        self.text = single_line(text);
        self.selection = self.text.len()..self.text.len();
        self.reversed = false;
        self.marked = None;
        self.undo.clear();
        self.redo.clear();
        self.last_edit = None;
    }

    fn clamp_boundary(&self, offset: usize) -> usize {
        let offset = offset.min(self.text.len());
        if self.is_boundary(offset) {
            offset
        } else {
            self.prev_grapheme(offset)
        }
    }

    fn is_boundary(&self, offset: usize) -> bool {
        offset == 0
            || offset == self.text.len()
            || self.text.grapheme_indices(true).any(|(i, _)| i == offset)
    }

    fn prev_grapheme(&self, offset: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .rev()
            .find_map(|(i, _)| (i < offset).then_some(i))
            .unwrap_or(0)
    }

    fn next_grapheme(&self, offset: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .find_map(|(i, _)| (i > offset).then_some(i))
            .unwrap_or(self.text.len())
    }

    /// The start of the word at or before `offset` (skipping spaces first).
    fn prev_word(&self, offset: usize) -> usize {
        self.text
            .split_word_bound_indices()
            .rev()
            .find(|(i, word)| *i < offset && word.chars().any(|c| !c.is_whitespace()))
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    /// The end of the word at or after `offset`.
    fn next_word(&self, offset: usize) -> usize {
        self.text
            .split_word_bound_indices()
            .map(|(i, word)| (i + word.len(), word))
            .find(|(end, word)| *end > offset && word.chars().any(|c| !c.is_whitespace()))
            .map(|(end, _)| end)
            .unwrap_or(self.text.len())
    }

    /// The word around `offset`, for double-click.
    pub fn word_at(&self, offset: usize) -> Range<usize> {
        self.text
            .split_word_bound_indices()
            .map(|(i, word)| i..i + word.len())
            .find(|range| {
                range.contains(&offset) || range.end == offset && offset == self.text.len()
            })
            .unwrap_or(offset..offset)
    }

    pub fn move_to(&mut self, offset: usize) {
        let offset = self.clamp_boundary(offset);
        self.selection = offset..offset;
        self.reversed = false;
        self.last_edit = None;
    }

    pub fn select_to(&mut self, offset: usize) {
        let offset = self.clamp_boundary(offset);
        if self.reversed {
            self.selection.start = offset;
        } else {
            self.selection.end = offset;
        }
        if self.selection.end < self.selection.start {
            self.reversed = !self.reversed;
            self.selection = self.selection.end..self.selection.start;
        }
        self.last_edit = None;
    }

    pub fn select_range(&mut self, range: Range<usize>) {
        let start = self.clamp_boundary(range.start);
        let end = self.clamp_boundary(range.end);
        self.selection = start.min(end)..start.max(end);
        self.reversed = false;
    }

    pub fn select_all(&mut self) {
        self.selection = 0..self.text.len();
        self.reversed = false;
    }

    pub fn left(&mut self, word: bool) {
        let target = if !self.selection.is_empty() && !word {
            self.selection.start
        } else if word {
            self.prev_word(self.cursor())
        } else {
            self.prev_grapheme(self.cursor())
        };
        self.move_to(target);
    }

    pub fn right(&mut self, word: bool) {
        let target = if !self.selection.is_empty() && !word {
            self.selection.end
        } else if word {
            self.next_word(self.cursor())
        } else {
            self.next_grapheme(self.cursor())
        };
        self.move_to(target);
    }

    pub fn select_left(&mut self, word: bool) {
        let target = if word {
            self.prev_word(self.cursor())
        } else {
            self.prev_grapheme(self.cursor())
        };
        self.select_to(target);
    }

    pub fn select_right(&mut self, word: bool) {
        let target = if word {
            self.next_word(self.cursor())
        } else {
            self.next_grapheme(self.cursor())
        };
        self.select_to(target);
    }

    /// Records an undo step, merging runs of typing or deleting.
    fn checkpoint(&mut self, kind: EditKind, now: Instant) {
        let merge = matches!(
            self.last_edit,
            Some((last, at)) if last == kind && kind != EditKind::Other && now.duration_since(at) < UNDO_COALESCE
        );
        if !merge {
            self.undo.push(Snapshot {
                text: self.text.clone(),
                selection: self.selection.clone(),
            });
            if self.undo.len() > UNDO_LIMIT {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.last_edit = Some((kind, now));
    }

    /// Replaces `range` (or the selection) with `new_text`, as typing or pasting
    /// does. Newlines become spaces; the max length truncates what's inserted.
    pub fn replace(&mut self, range: Option<Range<usize>>, new_text: &str, now: Instant) -> bool {
        let range = range
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection.clone());
        let range = self.clamp_boundary(range.start)..self.clamp_boundary(range.end);
        let mut new_text = single_line(new_text);
        if let Some(max) = self.max_len {
            let room = max.saturating_sub(
                self.text.chars().count() - self.text[range.clone()].chars().count(),
            );
            if new_text.chars().count() > room {
                new_text = new_text.chars().take(room).collect();
            }
        }
        if range.is_empty() && new_text.is_empty() {
            return false;
        }
        let kind = match (new_text.is_empty(), new_text.chars().count()) {
            (true, _) => EditKind::Deleting,
            (false, 1) if range.is_empty() => EditKind::Typing,
            _ => EditKind::Other,
        };
        if self.marked.is_some() {
            // Committing a composition: its start was already recorded, so the
            // whole composition undoes as one step.
            self.redo.clear();
            self.last_edit = Some((EditKind::Other, now));
        } else {
            self.checkpoint(kind, now);
        }
        self.text.replace_range(range.clone(), &new_text);
        let caret = range.start + new_text.len();
        self.selection = caret..caret;
        self.reversed = false;
        self.marked = None;
        true
    }

    /// Deletes the selection, or the grapheme/word before the caret.
    pub fn backspace(&mut self, word: bool, now: Instant) -> bool {
        if self.selection.is_empty() {
            let start = if word {
                self.prev_word(self.cursor())
            } else {
                self.prev_grapheme(self.cursor())
            };
            self.selection = start..self.cursor();
        }
        self.replace(None, "", now)
    }

    /// Deletes the selection, or the grapheme/word after the caret.
    pub fn delete(&mut self, word: bool, now: Instant) -> bool {
        if self.selection.is_empty() {
            let end = if word {
                self.next_word(self.cursor())
            } else {
                self.next_grapheme(self.cursor())
            };
            self.selection = self.cursor()..end;
        }
        self.replace(None, "", now)
    }

    /// IME: replaces `range` with uncommitted text and marks it.
    pub fn replace_and_mark(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        selected_in_new: Option<Range<usize>>,
    ) {
        let range = range
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection.clone());
        let range = range.start.min(self.text.len())..range.end.min(self.text.len());
        if self.marked.is_none() {
            // The composition as a whole is one undo step.
            self.checkpoint(EditKind::Other, Instant::now());
        }
        self.text.replace_range(range.clone(), new_text);
        self.marked = (!new_text.is_empty()).then(|| range.start..range.start + new_text.len());
        self.selection = match selected_in_new {
            Some(sel) => {
                range.start + sel.start.min(new_text.len())
                    ..range.start + sel.end.min(new_text.len())
            }
            None => range.start + new_text.len()..range.start + new_text.len(),
        };
        self.reversed = false;
    }

    pub fn unmark(&mut self) {
        self.marked = None;
    }

    pub fn undo(&mut self) -> bool {
        let Some(snapshot) = self.undo.pop() else {
            return false;
        };
        self.redo.push(Snapshot {
            text: std::mem::replace(&mut self.text, snapshot.text),
            selection: std::mem::replace(&mut self.selection, snapshot.selection),
        });
        self.marked = None;
        self.reversed = false;
        self.last_edit = None;
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(snapshot) = self.redo.pop() else {
            return false;
        };
        self.undo.push(Snapshot {
            text: std::mem::replace(&mut self.text, snapshot.text),
            selection: std::mem::replace(&mut self.selection, snapshot.selection),
        });
        self.marked = None;
        self.reversed = false;
        self.last_edit = None;
        true
    }

    // UTF-16 conversion for the platform input handler.

    pub fn offset_to_utf16(&self, offset: usize) -> usize {
        self.text[..offset.min(self.text.len())]
            .chars()
            .map(char::len_utf16)
            .sum()
    }

    pub fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf16 = 0;
        for (i, ch) in self.text.char_indices() {
            if utf16 >= offset {
                return i;
            }
            utf16 += ch.len_utf16();
        }
        self.text.len()
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
    }
}

fn single_line(text: &str) -> String {
    text.replace("\r\n", " ").replace(['\n', '\r'], " ")
}

// ---------------------------------------------------------------------------
// The view
// ---------------------------------------------------------------------------

/// Validation feedback: an error turns the frame red and shows the message.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum Validation {
    #[default]
    None,
    Error(SharedString),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Changed,
    /// Enter was pressed.
    Submitted,
    /// Focus left the input.
    Blurred,
}

pub struct TextInput {
    state: EditState,
    placeholder: SharedString,
    label: SharedString,
    validation: Validation,
    disabled: bool,
    focus_handle: FocusHandle,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    /// Horizontal scroll, so the caret stays visible in long text.
    scroll_x: Pixels,
    selecting: bool,
}

impl EventEmitter<Event> for TextInput {}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl TextInput {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        theme::observe(window, cx);
        let focus_handle = cx.focus_handle().tab_stop(true);
        cx.on_blur(&focus_handle, window, |this, _, cx| {
            this.selecting = false;
            cx.emit(Event::Blurred);
            cx.notify();
        })
        .detach();
        cx.on_focus(&focus_handle, window, |_, _, cx| cx.notify())
            .detach();
        Self {
            state: EditState::new(""),
            placeholder: SharedString::default(),
            label: SharedString::default(),
            validation: Validation::None,
            disabled: false,
            focus_handle,
            last_layout: None,
            last_bounds: None,
            scroll_x: px(0.),
            selecting: false,
        }
    }

    pub fn with_text(mut self, text: impl AsRef<str>) -> Self {
        self.state.set_text(text.as_ref());
        self
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Names the field for screen readers.
    pub fn with_label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    #[allow(dead_code)] // Kit API for later stages.
    pub fn with_max_len(mut self, max: usize) -> Self {
        self.state.max_len = Some(max);
        self
    }

    pub fn text(&self) -> &str {
        self.state.text()
    }

    pub fn set_text(&mut self, text: impl AsRef<str>, cx: &mut Context<Self>) {
        self.state.set_text(text.as_ref());
        cx.notify();
    }

    pub fn set_validation(&mut self, validation: Validation, cx: &mut Context<Self>) {
        if self.validation != validation {
            self.validation = validation;
            cx.notify();
        }
    }

    #[allow(dead_code)] // Kit API for later stages.
    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        self.focus_handle = self.focus_handle.clone().tab_stop(!disabled);
        cx.notify();
    }

    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus_handle.is_focused(window)
    }

    fn edited(&mut self, changed: bool, window: &mut Window, cx: &mut Context<Self>) {
        if changed {
            window.notify_text_input_changed(&self.focus_handle);
            cx.emit(Event::Changed);
        }
        cx.notify();
    }

    fn moved(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify();
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        let changed = self.state.backspace(false, Instant::now());
        self.edited(changed, window, cx);
    }
    fn backspace_word(&mut self, _: &BackspaceWord, window: &mut Window, cx: &mut Context<Self>) {
        let changed = self.state.backspace(true, Instant::now());
        self.edited(changed, window, cx);
    }
    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        let changed = self.state.delete(false, Instant::now());
        self.edited(changed, window, cx);
    }
    fn delete_word(&mut self, _: &DeleteWord, window: &mut Window, cx: &mut Context<Self>) {
        let changed = self.state.delete(true, Instant::now());
        self.edited(changed, window, cx);
    }
    fn left(&mut self, _: &Left, window: &mut Window, cx: &mut Context<Self>) {
        self.state.left(false);
        self.moved(window, cx);
    }
    fn right(&mut self, _: &Right, window: &mut Window, cx: &mut Context<Self>) {
        self.state.right(false);
        self.moved(window, cx);
    }
    fn word_left(&mut self, _: &WordLeft, window: &mut Window, cx: &mut Context<Self>) {
        self.state.left(true);
        self.moved(window, cx);
    }
    fn word_right(&mut self, _: &WordRight, window: &mut Window, cx: &mut Context<Self>) {
        self.state.right(true);
        self.moved(window, cx);
    }
    fn select_left(&mut self, _: &SelectLeft, window: &mut Window, cx: &mut Context<Self>) {
        self.state.select_left(false);
        self.moved(window, cx);
    }
    fn select_right(&mut self, _: &SelectRight, window: &mut Window, cx: &mut Context<Self>) {
        self.state.select_right(false);
        self.moved(window, cx);
    }
    fn select_word_left(
        &mut self,
        _: &SelectWordLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.select_left(true);
        self.moved(window, cx);
    }
    fn select_word_right(
        &mut self,
        _: &SelectWordRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.select_right(true);
        self.moved(window, cx);
    }
    fn select_all(&mut self, _: &SelectAll, window: &mut Window, cx: &mut Context<Self>) {
        self.state.select_all();
        self.moved(window, cx);
    }
    fn home(&mut self, _: &Home, window: &mut Window, cx: &mut Context<Self>) {
        self.state.move_to(0);
        self.moved(window, cx);
    }
    fn end(&mut self, _: &End, window: &mut Window, cx: &mut Context<Self>) {
        self.state.move_to(usize::MAX);
        self.moved(window, cx);
    }
    fn select_to_home(&mut self, _: &SelectToHome, window: &mut Window, cx: &mut Context<Self>) {
        self.state.select_to(0);
        self.moved(window, cx);
    }
    fn select_to_end(&mut self, _: &SelectToEnd, window: &mut Window, cx: &mut Context<Self>) {
        self.state.select_to(usize::MAX);
        self.moved(window, cx);
    }
    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let changed = self.state.replace(None, &text, Instant::now());
            self.edited(changed, window, cx);
        }
    }
    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        let selected = self.state.selected_text();
        if !selected.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(selected.to_owned()));
        }
    }
    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        let selected = self.state.selected_text();
        if !selected.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(selected.to_owned()));
            let changed = self.state.replace(None, "", Instant::now());
            self.edited(changed, window, cx);
        }
    }
    fn undo(&mut self, _: &Undo, window: &mut Window, cx: &mut Context<Self>) {
        let changed = self.state.undo();
        self.edited(changed, window, cx);
    }
    fn redo(&mut self, _: &Redo, window: &mut Window, cx: &mut Context<Self>) {
        let changed = self.state.redo();
        self.edited(changed, window, cx);
    }
    fn submit(&mut self, _: &Submit, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(Event::Submitted);
        // Let a dialog's Enter handling see it too.
        cx.propagate();
    }
    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    fn index_for_position(&self, position: Point<Pixels>) -> usize {
        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if self.state.text.is_empty() {
            return 0;
        }
        let x = position.x - bounds.left() + self.scroll_x;
        line.closest_index_for_x(x)
    }

    fn on_mouse_down(
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

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selecting {
            self.state
                .select_to(self.index_for_position(event.position));
            self.moved(window, cx);
        }
    }
}

impl gpui::EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.state.range_from_utf16(&range_utf16);
        actual_range.replace(self.state.range_to_utf16(&range));
        Some(self.state.text[range].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.state.range_to_utf16(&self.state.selection),
            reversed: self.state.reversed,
        })
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = self.state.range_from_utf16(&range_utf16);
        self.state.select_range(range);
        self.moved(window, cx);
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.state.offset_to_utf16(self.state.text.len()))
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.state
            .marked
            .as_ref()
            .map(|range| self.state.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.state.unmark();
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        let range = range_utf16.map(|r| self.state.range_from_utf16(&r));
        let changed = self.state.replace(range, new_text, Instant::now());
        self.edited(changed, window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        let range = range_utf16.map(|r| self.state.range_from_utf16(&r));
        // The selection inside the new text arrives in UTF-16 too.
        let selected = new_selected_range_utf16.map(|sel| {
            let to_utf8 = |n: usize| {
                let mut utf16 = 0;
                for (i, ch) in new_text.char_indices() {
                    if utf16 >= n {
                        return i;
                    }
                    utf16 += ch.len_utf16();
                }
                new_text.len()
            };
            to_utf8(sel.start)..to_utf8(sel.end)
        });
        self.state.replace_and_mark(range, new_text, selected);
        self.edited(true, window, cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let line = self.last_layout.as_ref()?;
        let range = self.state.range_from_utf16(&range_utf16);
        Some(Bounds::from_corners(
            point(
                bounds.left() + line.x_for_index(range.start) - self.scroll_x,
                bounds.top(),
            ),
            point(
                bounds.left() + line.x_for_index(range.end) - self.scroll_x,
                bounds.bottom(),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let local = self.last_bounds?.localize(&point)?;
        let line = self.last_layout.as_ref()?;
        let index = line.closest_index_for_x(local.x + self.scroll_x);
        Some(self.state.offset_to_utf16(index))
    }
}

/// Paints the text, selection, caret and IME underline, and registers the
/// platform input handler.
struct TextElement {
    input: gpui::Entity<TextInput>,
}

struct PrepaintState {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
    scroll_x: Pixels,
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> PrepaintState {
        let p = theme::palette(window, cx);
        let input = self.input.read(cx);
        let state = &input.state;
        let style = window.text_style();
        let focused = input.focus_handle.is_focused(window);

        let (text, color) = if state.text.is_empty() {
            (input.placeholder.clone(), p.text_muted.opacity(0.7))
        } else {
            (SharedString::from(state.text.clone()), style.color)
        };
        let run = TextRun {
            len: text.len(),
            font: style.font(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = match state.marked.as_ref().filter(|_| !state.text.is_empty()) {
            Some(marked) => [
                TextRun {
                    len: marked.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked.end - marked.start,
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: text.len() - marked.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect(),
            None => vec![run],
        };
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(text, font_size, &runs, None);

        // Keep the caret in view.
        let caret_x = if state.text.is_empty() {
            px(0.)
        } else {
            line.x_for_index(state.cursor())
        };
        let width = bounds.size.width;
        let mut scroll_x = input.scroll_x;
        if caret_x - scroll_x > width - px(2.) {
            scroll_x = caret_x - width + px(2.);
        } else if caret_x < scroll_x {
            scroll_x = caret_x;
        }
        scroll_x = scroll_x
            .max(px(0.))
            .min((line.width - width + px(2.)).max(px(0.)));

        let selection = state.selection.clone();
        let (selection, cursor) = if selection.is_empty() || state.text.is_empty() {
            let cursor = focused.then(|| {
                fill(
                    Bounds::new(
                        point(bounds.left() + caret_x - scroll_x, bounds.top()),
                        size(px(1.5), bounds.size.height),
                    ),
                    p.accent_fg,
                )
            });
            (None, cursor)
        } else {
            let quad = fill(
                Bounds::from_corners(
                    point(
                        bounds.left() + line.x_for_index(selection.start) - scroll_x,
                        bounds.top(),
                    ),
                    point(
                        bounds.left() + line.x_for_index(selection.end) - scroll_x,
                        bounds.bottom(),
                    ),
                ),
                if focused {
                    p.selection
                } else {
                    p.selection.opacity(0.15)
                },
            );
            (Some(quad), None)
        };
        PrepaintState {
            line: Some(line),
            cursor,
            selection,
            scroll_x,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        prepaint: &mut PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        let scroll_x = prepaint.scroll_x;
        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
            if let Some(selection) = prepaint.selection.take() {
                window.paint_quad(selection);
            }
            if let Some(line) = prepaint.line.as_ref() {
                line.paint(
                    point(bounds.left() - scroll_x, bounds.top()),
                    window.line_height(),
                    gpui::TextAlign::Left,
                    None,
                    window,
                    cx,
                )
                .ok();
            }
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        });
        let line = prepaint.line.take();
        self.input.update(cx, |input, _| {
            input.last_layout = line;
            input.last_bounds = Some(bounds);
            input.scroll_x = scroll_x;
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let error = match &self.validation {
            Validation::Error(message) => Some(message.clone()),
            Validation::None => None,
        };
        let border = if error.is_some() { p.danger } else { p.well };
        let enabled = !self.disabled;

        let field = div()
            .id("text-input")
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .role(accesskit::Role::TextInput)
            .aria_label(self.label.clone())
            .aria_value(SharedString::from(self.state.text.clone()))
            .aria_placeholder(self.placeholder.clone())
            .h(px(32.))
            .w_full()
            .flex()
            .items_center()
            .px(px(10.))
            .rounded(px(metrics::RADIUS))
            .bg(p.well)
            .border_2()
            .border_color(border)
            .text_size(px(14.))
            .line_height(px(20.))
            .text_color(p.text)
            .when(enabled, |f| {
                f.cursor(CursorStyle::IBeam)
                    .on_action(cx.listener(Self::backspace))
                    .on_action(cx.listener(Self::backspace_word))
                    .on_action(cx.listener(Self::delete))
                    .on_action(cx.listener(Self::delete_word))
                    .on_action(cx.listener(Self::left))
                    .on_action(cx.listener(Self::right))
                    .on_action(cx.listener(Self::word_left))
                    .on_action(cx.listener(Self::word_right))
                    .on_action(cx.listener(Self::select_left))
                    .on_action(cx.listener(Self::select_right))
                    .on_action(cx.listener(Self::select_word_left))
                    .on_action(cx.listener(Self::select_word_right))
                    .on_action(cx.listener(Self::select_all))
                    .on_action(cx.listener(Self::home))
                    .on_action(cx.listener(Self::end))
                    .on_action(cx.listener(Self::select_to_home))
                    .on_action(cx.listener(Self::select_to_end))
                    .on_action(cx.listener(Self::paste))
                    .on_action(cx.listener(Self::cut))
                    .on_action(cx.listener(Self::copy))
                    .on_action(cx.listener(Self::undo))
                    .on_action(cx.listener(Self::redo))
                    .on_action(cx.listener(Self::submit))
                    .on_action(cx.listener(Self::show_character_palette))
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
                    .on_mouse_move(cx.listener(Self::on_mouse_move))
            })
            .when(!enabled, |f| f.opacity(0.4))
            .child(TextElement { input: cx.entity() });
        let field = if error.is_some() {
            field
        } else {
            focus::ring(field, &p)
        };

        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .w_full()
            .child(field)
            .when_some(error, |this, message| {
                this.child(
                    div()
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .text_color(p.danger)
                        .child(message),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(ms: u64) -> Instant {
        thread_local!(static T0: Instant = Instant::now());
        T0.with(|t0| *t0 + Duration::from_millis(ms))
    }

    fn typed(text: &str) -> EditState {
        let mut s = EditState::new("");
        for (i, ch) in text.chars().enumerate() {
            s.replace(None, &ch.to_string(), at(i as u64 * 10));
        }
        s
    }

    #[test]
    fn typing_and_backspace_respect_graphemes() {
        // "e" + combining acute, and a family emoji (one grapheme, many chars).
        let mut s = EditState::new("cafe\u{301} 👨‍👩‍👧");
        assert!(s.backspace(false, at(0)));
        assert_eq!(s.text(), "cafe\u{301} ");
        s.backspace(false, at(10));
        s.backspace(false, at(20));
        assert_eq!(s.text(), "caf");
    }

    #[test]
    fn arrows_move_by_grapheme_and_collapse_selection() {
        let mut s = EditState::new("ab\u{301}c");
        s.move_to(0);
        s.right(false);
        s.right(false);
        assert_eq!(s.cursor(), "ab\u{301}".len());
        s.select_all();
        s.left(false);
        assert_eq!(s.selection(), 0..0);
        s.select_all();
        s.right(false);
        assert_eq!(s.cursor(), s.text().len());
    }

    #[test]
    fn word_movement() {
        let mut s = EditState::new("open   network stream");
        s.left(true);
        assert_eq!(s.cursor(), "open   network ".len());
        s.left(true);
        assert_eq!(s.cursor(), "open   ".len());
        s.left(true);
        assert_eq!(s.cursor(), 0);
        s.right(true);
        assert_eq!(s.cursor(), "open".len());
        s.right(true);
        assert_eq!(s.cursor(), "open   network".len());
    }

    #[test]
    fn word_deletion() {
        let mut s = EditState::new("rock and roll");
        s.backspace(true, at(0));
        assert_eq!(s.text(), "rock and ");
        s.move_to(0);
        s.delete(true, at(10));
        assert_eq!(s.text(), " and ");
    }

    #[test]
    fn shift_selection_flips_direction() {
        let mut s = EditState::new("abcd");
        s.move_to(2);
        s.select_right(false);
        assert_eq!(s.selection(), 2..3);
        s.select_left(false);
        s.select_left(false);
        assert_eq!(s.selection(), 1..2);
        assert_eq!(s.cursor(), 1);
        assert_eq!(s.selected_text(), "b");
    }

    #[test]
    fn typing_replaces_the_selection() {
        let mut s = EditState::new("hello world");
        s.select_range(0..5);
        s.replace(None, "goodbye", at(0));
        assert_eq!(s.text(), "goodbye world");
        assert_eq!(s.selection(), 7..7);
    }

    #[test]
    fn pasted_newlines_become_spaces() {
        let mut s = EditState::new("");
        s.replace(None, "a\r\nb\nc", at(0));
        assert_eq!(s.text(), "a b c");
    }

    #[test]
    fn undo_groups_typing_and_redo_restores() {
        let mut s = typed("abc");
        // One pause, then more typing: two undo steps.
        s.replace(None, "d", at(5000));
        assert!(s.undo());
        assert_eq!(s.text(), "abc");
        assert!(s.undo());
        assert_eq!(s.text(), "");
        assert!(!s.undo());
        assert!(s.redo());
        assert_eq!(s.text(), "abc");
        // A new edit drops the redo history.
        s.replace(None, "x", at(9000));
        assert!(!s.redo());
    }

    #[test]
    fn undo_separates_typing_from_deleting() {
        let mut s = typed("ab");
        s.backspace(false, at(30));
        assert_eq!(s.text(), "a");
        s.undo();
        assert_eq!(s.text(), "ab");
    }

    #[test]
    fn max_length_truncates_insertions() {
        let mut s = EditState::new("12");
        s.max_len = Some(4);
        s.replace(None, "3456", at(0));
        assert_eq!(s.text(), "1234");
        // Replacing a selection frees its room.
        s.select_range(0..4);
        s.replace(None, "abcdef", at(10));
        assert_eq!(s.text(), "abcd");
    }

    #[test]
    fn ime_composition_marks_and_commits() {
        let mut s = EditState::new("x");
        s.replace_and_mark(None, "ni", None);
        assert_eq!(s.text(), "xni");
        assert_eq!(s.marked, Some(1..3));
        s.replace_and_mark(None, "你", None);
        assert_eq!(s.text(), "x你");
        s.replace(None, "你好", at(0));
        assert_eq!(s.text(), "x你好");
        assert_eq!(s.marked, None);
        // The whole composition undoes as one step.
        s.undo();
        assert_eq!(s.text(), "x");
    }

    #[test]
    fn utf16_round_trip() {
        let s = EditState::new("a😀b");
        assert_eq!(s.offset_to_utf16(s.text().len()), 4);
        assert_eq!(s.offset_from_utf16(3), "a😀".len());
        assert_eq!(s.offset_from_utf16(99), s.text().len());
    }

    #[test]
    fn double_click_selects_a_word() {
        let s = EditState::new("Always starts at");
        assert_eq!(s.word_at(9), 7..13);
        assert_eq!(s.word_at(16), 14..16);
    }

    #[test]
    fn right_to_left_text_is_edited_by_grapheme() {
        // Hebrew with a combining point: backspace removes the letter and point.
        let mut s = EditState::new("שָׁלוֹם");
        let before = s.text().chars().count();
        s.backspace(false, at(0));
        assert!(s.text().chars().count() < before);
        assert!(s.text().is_char_boundary(s.text().len()));
    }
}
