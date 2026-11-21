//! Conversion between UTF-8 byte offsets and the UTF-16 offsets platform IME calls use.

use std::ops::Range;

/// The UTF-16 offset of the UTF-8 `offset` in `text` (clamped to its end).
pub(super) fn offset_to_utf16(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())].chars().map(char::len_utf16).sum()
}

/// The UTF-8 offset of the UTF-16 `offset` in `text` (clamped to its end).
pub(super) fn offset_from_utf16(text: &str, offset: usize) -> usize {
    let mut utf16 = 0;
    for (i, ch) in text.char_indices() {
        if utf16 >= offset {
            return i;
        }
        utf16 += ch.len_utf16();
    }
    text.len()
}

/// [`offset_to_utf16`] for both ends of a range.
pub(super) fn range_to_utf16(text: &str, range: &Range<usize>) -> Range<usize> {
    offset_to_utf16(text, range.start)..offset_to_utf16(text, range.end)
}

/// [`offset_from_utf16`] for both ends of a range.
pub(super) fn range_from_utf16(text: &str, range: &Range<usize>) -> Range<usize> {
    offset_from_utf16(text, range.start)..offset_from_utf16(text, range.end)
}
