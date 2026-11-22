//! Where a search's words appear in a row's label (S5.2.3), so the page can mark them. Matching
//! ignores accents and case, as the search does, but the ranges are in the label as written.

use std::ops::Range;

use crate::library::search::fold;

/// The byte ranges of `text` that any of `words` (folded, from
/// [`search_words`](crate::settings::registry::search_words)) match, merged and in order.
pub(super) fn ranges(text: &str, words: &[String]) -> Vec<Range<usize>> {
    let (folded, origin) = fold_with_origin(text);
    let mut found: Vec<Range<usize>> = Vec::new();
    for word in words.iter().filter(|word| !word.is_empty()) {
        for (start, _) in folded.match_indices(word.as_str()) {
            let end = start + word.len();
            found.push(origin[start].start..origin[end - 1].end);
        }
    }
    merge(found)
}

/// `text` folded, and for each byte of the folded text the range of the character in `text` it
/// came from.
fn fold_with_origin(text: &str) -> (String, Vec<Range<usize>>) {
    let mut folded = String::with_capacity(text.len());
    let mut origin = Vec::with_capacity(text.len());
    let mut buffer = [0; 4];
    for (index, character) in text.char_indices() {
        let piece = fold(character.encode_utf8(&mut buffer));
        origin.extend(std::iter::repeat_n(index..index + character.len_utf8(), piece.len()));
        folded.push_str(&piece);
    }
    (folded, origin)
}

/// `ranges` sorted, with overlapping and touching ranges joined.
fn merge(mut ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    ranges.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
    for range in ranges {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::ranges;
    use crate::settings::registry::search_words;

    fn marked(text: &str, query: &str) -> Vec<&'static str> {
        let text: &'static str = Box::leak(text.to_owned().into_boxed_str());
        ranges(text, &search_words(query)).into_iter().map(|range| &text[range]).collect()
    }

    #[test]
    fn marks_each_word_ignoring_case() {
        assert_eq!(marked("Hide the controls after", "CONTROLS hide"), ["Hide", "controls"]);
    }

    #[test]
    fn marks_accented_letters_in_the_label_as_written() {
        assert_eq!(marked("Préférences de lecture", "prefer"), ["Préfér"]);
    }

    #[test]
    fn joins_overlapping_matches() {
        assert_eq!(marked("subtitles", "sub subtitle"), ["subtitle"]);
    }

    #[test]
    fn marks_nothing_without_a_match() {
        assert_eq!(marked("Theme", "volume"), Vec::<&str>::new());
    }
}
