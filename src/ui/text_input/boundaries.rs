//! Grapheme and word boundaries in a line of text.
//!
//! Offsets are UTF-8 byte offsets. The caret only ever rests on grapheme boundaries, so a
//! combining mark or an emoji sequence is never split.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

/// Whether `offset` lies on a grapheme boundary of `text`.
pub(super) fn is_boundary(text: &str, offset: usize) -> bool {
    offset == 0 || offset == text.len() || text.grapheme_indices(true).any(|(i, _)| i == offset)
}

/// Moves `offset` into `text` and back onto the grapheme boundary at or before it.
pub(super) fn clamp_boundary(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    if is_boundary(text, offset) { offset } else { prev_grapheme(text, offset) }
}

/// The grapheme boundary before `offset`, or 0.
pub(super) fn prev_grapheme(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true).rev().find_map(|(i, _)| (i < offset).then_some(i)).unwrap_or(0)
}

/// The grapheme boundary after `offset`, or the end of the text.
pub(super) fn next_grapheme(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true).find_map(|(i, _)| (i > offset).then_some(i)).unwrap_or(text.len())
}

/// The start of the word at or before `offset` (skipping spaces first).
pub(super) fn prev_word(text: &str, offset: usize) -> usize {
    text.split_word_bound_indices()
        .rev()
        .find(|(i, word)| *i < offset && has_content(word))
        .map_or(0, |(i, _)| i)
}

/// The end of the word at or after `offset`.
pub(super) fn next_word(text: &str, offset: usize) -> usize {
    text.split_word_bound_indices()
        .map(|(i, word)| (i + word.len(), word))
        .find(|(end, word)| *end > offset && has_content(word))
        .map_or(text.len(), |(end, _)| end)
}

/// The word around `offset`, for double-click.
pub(super) fn word_at(text: &str, offset: usize) -> Range<usize> {
    text.split_word_bound_indices()
        .map(|(i, word)| i..i + word.len())
        .find(|range| range.contains(&offset) || range.end == offset && offset == text.len())
        .unwrap_or(offset..offset)
}

/// Whether a word segment is a real word rather than a run of spaces.
fn has_content(word: &str) -> bool {
    word.chars().any(|c| !c.is_whitespace())
}
