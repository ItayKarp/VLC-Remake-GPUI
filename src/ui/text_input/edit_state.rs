//! The editing rules of a text input, single-line or multi-line, with no GPUI types so they're
//! unit tested.

use std::ops::Range;
use std::time::Instant;

use super::boundaries::{
    clamp_boundary, next_grapheme, next_word, prev_grapheme, prev_word, word_at,
};
use super::history::{EditKind, History, Snapshot};
use super::utf16;

/// Text, selection and undo history of a text input.
#[derive(Clone, Debug, Default)]
pub struct EditState {
    text: String,
    /// Always on grapheme boundaries; `start <= end`.
    selection: Range<usize>,
    /// The caret is at `selection.start` instead of `end`.
    reversed: bool,
    /// IME composition in progress.
    marked: Option<Range<usize>>,
    history: History,
    max_len: Option<usize>,
    /// Line breaks are kept (as `\n`) instead of becoming spaces.
    multi_line: bool,
}

impl EditState {
    /// Creates a state holding `text` (newlines become spaces) with the caret at its end.
    pub fn new(text: &str) -> Self {
        let text = single_line(text);
        let end = text.len();
        Self { text, selection: end..end, ..Default::default() }
    }

    /// The current text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The selection as UTF-8 byte offsets; empty when there's only a caret.
    pub fn selection(&self) -> Range<usize> {
        self.selection.clone()
    }

    /// Whether the caret sits at the start of the selection rather than its end.
    pub fn is_reversed(&self) -> bool {
        self.reversed
    }

    /// The uncommitted IME composition, if one is in progress.
    pub fn marked(&self) -> Option<&Range<usize>> {
        self.marked.as_ref()
    }

    /// Keeps line breaks from now on: the input holds several lines.
    pub fn set_multi_line(&mut self) {
        self.multi_line = true;
    }

    /// Whether line breaks are kept.
    pub fn is_multi_line(&self) -> bool {
        self.multi_line
    }

    /// Limits the text to `max` characters; insertions past it are truncated.
    pub fn set_max_len(&mut self, max: usize) {
        self.max_len = Some(max);
    }

    /// The caret's offset.
    pub fn cursor(&self) -> usize {
        if self.reversed { self.selection.start } else { self.selection.end }
    }

    /// The text inside the selection.
    pub fn selected_text(&self) -> &str {
        &self.text[self.selection.clone()]
    }

    /// Replaces the whole text (from outside, not the user); clears history.
    pub fn set_text(&mut self, text: &str) {
        self.text = self.normalize(text);
        self.selection = self.text.len()..self.text.len();
        self.reversed = false;
        self.marked = None;
        self.history.clear();
    }

    /// The word around `offset`, for double-click.
    pub fn word_at(&self, offset: usize) -> Range<usize> {
        word_at(&self.text, offset)
    }

    /// Collapses the selection to a caret at `offset`.
    pub fn move_to(&mut self, offset: usize) {
        let offset = clamp_boundary(&self.text, offset);
        self.selection = offset..offset;
        self.reversed = false;
        self.history.break_run();
    }

    /// Moves the caret to `offset`, keeping the selection's other end where it is.
    pub fn select_to(&mut self, offset: usize) {
        let offset = clamp_boundary(&self.text, offset);
        if self.reversed {
            self.selection.start = offset;
        } else {
            self.selection.end = offset;
        }
        if self.selection.end < self.selection.start {
            self.reversed = !self.reversed;
            self.selection = self.selection.end..self.selection.start;
        }
        self.history.break_run();
    }

    /// Selects `range`, snapped to grapheme boundaries, with the caret at its end.
    pub fn select_range(&mut self, range: Range<usize>) {
        let start = clamp_boundary(&self.text, range.start);
        let end = clamp_boundary(&self.text, range.end);
        self.selection = start.min(end)..start.max(end);
        self.reversed = false;
    }

    /// Selects the whole text.
    pub fn select_all(&mut self) {
        self.selection = 0..self.text.len();
        self.reversed = false;
    }

    /// Left arrow: collapses a selection to its start, or moves back a grapheme or `word`.
    pub fn left(&mut self, word: bool) {
        let target = if !self.selection.is_empty() && !word {
            self.selection.start
        } else {
            self.prev_stop(word)
        };
        self.move_to(target);
    }

    /// Right arrow: collapses a selection to its end, or moves on a grapheme or `word`.
    pub fn right(&mut self, word: bool) {
        let target = if !self.selection.is_empty() && !word {
            self.selection.end
        } else {
            self.next_stop(word)
        };
        self.move_to(target);
    }

    /// Shift+Left: extends the selection back a grapheme or `word`.
    pub fn select_left(&mut self, word: bool) {
        self.select_to(self.prev_stop(word));
    }

    /// Shift+Right: extends the selection on a grapheme or `word`.
    pub fn select_right(&mut self, word: bool) {
        self.select_to(self.next_stop(word));
    }

    /// The previous caret stop: the start of a word, or the previous grapheme.
    fn prev_stop(&self, word: bool) -> usize {
        if word {
            prev_word(&self.text, self.cursor())
        } else {
            prev_grapheme(&self.text, self.cursor())
        }
    }

    /// The next caret stop: the end of a word, or the next grapheme.
    fn next_stop(&self, word: bool) -> usize {
        if word {
            next_word(&self.text, self.cursor())
        } else {
            next_grapheme(&self.text, self.cursor())
        }
    }

    /// Replaces `range` (or the selection) with `new_text`, as typing or pasting
    /// does. In a single-line input newlines become spaces; the max length truncates what's
    /// inserted.
    pub fn replace(&mut self, range: Option<Range<usize>>, new_text: &str, now: Instant) -> bool {
        let range = range.or_else(|| self.marked.clone()).unwrap_or_else(|| self.selection.clone());
        let range = clamp_boundary(&self.text, range.start)..clamp_boundary(&self.text, range.end);
        let new_text = self.fit_to_room(self.normalize(new_text), &range);
        if range.is_empty() && new_text.is_empty() {
            return false;
        }
        if self.marked.is_some() {
            self.history.commit_composition(now);
        } else {
            let kind = edit_kind(&range, &new_text);
            self.history.checkpoint(kind, now, &self.text, &self.selection);
        }
        self.text.replace_range(range.clone(), &new_text);
        let caret = range.start + new_text.len();
        self.selection = caret..caret;
        self.reversed = false;
        self.marked = None;
        true
    }

    /// `text` with its line breaks as this input keeps them: `\n` in a multi-line input,
    /// spaces in a single-line one.
    fn normalize(&self, text: &str) -> String {
        if self.multi_line {
            text.replace("\r\n", "\n").replace('\r', "\n")
        } else {
            single_line(text)
        }
    }

    /// Where the line holding `offset` starts: 0 in a single-line input.
    pub fn line_start(&self, offset: usize) -> usize {
        let offset = offset.min(self.text.len());
        self.text[..offset].rfind('\n').map_or(0, |at| at + 1)
    }

    /// Where the line holding `offset` ends, before its line break: the text's end in a
    /// single-line input.
    pub fn line_end(&self, offset: usize) -> usize {
        let offset = offset.min(self.text.len());
        self.text[offset..].find('\n').map_or(self.text.len(), |at| offset + at)
    }

    /// Truncates `new_text` so replacing `range` with it keeps within the max length.
    fn fit_to_room(&self, new_text: String, range: &Range<usize>) -> String {
        let Some(max) = self.max_len else {
            return new_text;
        };
        let kept = self.text.chars().count() - self.text[range.clone()].chars().count();
        let room = max.saturating_sub(kept);
        if new_text.chars().count() > room {
            new_text.chars().take(room).collect()
        } else {
            new_text
        }
    }

    /// Deletes the selection, or the grapheme/word before the caret.
    pub fn backspace(&mut self, word: bool, now: Instant) -> bool {
        if self.selection.is_empty() {
            self.selection = self.prev_stop(word)..self.cursor();
        }
        self.replace(None, "", now)
    }

    /// Deletes the selection, or the grapheme/word after the caret.
    pub fn delete(&mut self, word: bool, now: Instant) -> bool {
        if self.selection.is_empty() {
            self.selection = self.cursor()..self.next_stop(word);
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
        let range = range.or_else(|| self.marked.clone()).unwrap_or_else(|| self.selection.clone());
        let range = range.start.min(self.text.len())..range.end.min(self.text.len());
        if self.marked.is_none() {
            // The composition as a whole is one undo step.
            self.history.checkpoint(EditKind::Other, Instant::now(), &self.text, &self.selection);
        }
        self.text.replace_range(range.clone(), new_text);
        self.marked = (!new_text.is_empty()).then(|| range.start..range.start + new_text.len());
        let len = new_text.len();
        let selected = selected_in_new.map_or(len..len, |sel| sel.start.min(len)..sel.end.min(len));
        self.selection = range.start + selected.start..range.start + selected.end;
        self.reversed = false;
    }

    /// Ends the IME composition, keeping its text.
    pub fn unmark(&mut self) {
        self.marked = None;
    }

    /// Restores the state before the last undo step; false when there's none.
    pub fn undo(&mut self) -> bool {
        let Some(snapshot) = self.history.pop_undo() else {
            return false;
        };
        let replaced = self.restore(snapshot);
        self.history.push_redo(replaced);
        true
    }

    /// Re-applies the last undone step; false when there's none.
    pub fn redo(&mut self) -> bool {
        let Some(snapshot) = self.history.pop_redo() else {
            return false;
        };
        let replaced = self.restore(snapshot);
        self.history.push_undo(replaced);
        true
    }

    /// Swaps in `snapshot`'s text and selection and returns the ones it replaced.
    fn restore(&mut self, snapshot: Snapshot) -> Snapshot {
        let replaced = Snapshot {
            text: std::mem::replace(&mut self.text, snapshot.text),
            selection: std::mem::replace(&mut self.selection, snapshot.selection),
        };
        self.marked = None;
        self.reversed = false;
        self.history.break_run();
        replaced
    }

    /// The UTF-16 offset of the UTF-8 `offset`, for the platform input handler.
    pub fn offset_to_utf16(&self, offset: usize) -> usize {
        utf16::offset_to_utf16(&self.text, offset)
    }

    /// The UTF-8 offset of the UTF-16 `offset`; the input handler converts whole ranges instead.
    #[cfg(test)]
    pub fn offset_from_utf16(&self, offset: usize) -> usize {
        utf16::offset_from_utf16(&self.text, offset)
    }

    /// [`Self::offset_to_utf16`] for both ends of a range.
    pub(super) fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        utf16::range_to_utf16(&self.text, range)
    }

    /// The UTF-8 offsets of both ends of a UTF-16 range, for the platform input handler.
    pub(super) fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        utf16::range_from_utf16(&self.text, range)
    }
}

/// Classifies an edit that replaces `range` with `new_text`, for undo coalescing.
fn edit_kind(range: &Range<usize>, new_text: &str) -> EditKind {
    match (new_text.is_empty(), new_text.chars().count()) {
        (true, _) => EditKind::Deleting,
        (false, 1) if range.is_empty() => EditKind::Typing,
        _ => EditKind::Other,
    }
}

/// Turns line breaks into spaces: the input holds a single line.
fn single_line(text: &str) -> String {
    text.replace("\r\n", " ").replace(['\n', '\r'], " ")
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

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

    #[test]
    fn a_multi_line_input_keeps_line_breaks() {
        let mut s = EditState::new("");
        s.set_multi_line();
        s.replace(None, "one\r\ntwo\rthree", at(0));
        assert_eq!(s.text(), "one\ntwo\nthree");
        assert_eq!((s.line_start(5), s.line_end(5)), (4, 7));
        assert_eq!((s.line_start(0), s.line_end(0)), (0, 3));
        assert_eq!(s.line_end(99), s.text().len());
        let single = EditState::new("one\ntwo");
        assert_eq!((single.line_start(5), single.line_end(0)), (0, 7));
    }
}
