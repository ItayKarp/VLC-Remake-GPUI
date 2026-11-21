//! Type-to-select for lists and menus: typed letters form a prefix query, and
//! repeating one letter cycles through the items that start with it.

use std::time::{Duration, Instant};

/// Type-to-select: letters typed in quick succession form a prefix query.
#[derive(Default)]
pub struct TypeAhead {
    query: String,
    last: Option<Instant>,
}

impl TypeAhead {
    /// Pause after which typing starts a new query.
    pub const RESET_AFTER: Duration = Duration::from_millis(1000);

    /// Adds typed text and returns the query to search for.
    pub fn push(&mut self, text: &str, now: Instant) -> String {
        if self.last.is_none_or(|last| now.duration_since(last) > Self::RESET_AFTER) {
            self.query.clear();
        }
        self.last = Some(now);
        self.query.push_str(&text.to_lowercase());
        self.query.clone()
    }
}

/// The item to jump to for `query`, searching after `current`.
///
/// A query of one repeated letter ("a", "aa", "aaa") cycles through the items
/// starting with that letter; anything else finds the first item starting with
/// the query, from `current` on, wrapping. Matching ignores case.
pub fn find_match(
    labels: &[&str],
    query: &str,
    current: Option<usize>,
    enabled: impl Fn(usize) -> bool,
) -> Option<usize> {
    let len = labels.len();
    if len == 0 || query.is_empty() {
        return None;
    }
    let mut chars = query.chars();
    let first = chars.next()?;
    let repeated = chars.all(|c| c == first);
    let (needle, start) = if repeated {
        // Cycle: start after the current item.
        (first.to_string(), current.map_or(0, |c| c + 1))
    } else {
        // Refine: the current item still counts if it matches the longer prefix.
        (query.to_owned(), current.unwrap_or(0))
    };
    (0..len)
        .map(|offset| (start + offset) % len)
        .find(|&i| enabled(i) && labels[i].to_lowercase().starts_with(&needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LABELS: &[&str] = &["Auto", "Bass", "Bright", "Classical", "Club", "Dance"];

    fn all(_: usize) -> bool {
        true
    }

    #[test]
    fn prefix_match_ignores_case() {
        assert_eq!(find_match(LABELS, "cla", None, all), Some(3));
        assert_eq!(find_match(LABELS, "clu", Some(3), all), Some(4));
        assert_eq!(find_match(LABELS, "x", None, all), None);
    }

    #[test]
    fn repeating_a_letter_cycles() {
        assert_eq!(find_match(LABELS, "b", None, all), Some(1));
        assert_eq!(find_match(LABELS, "bb", Some(1), all), Some(2));
        // Wraps back to the first "B" item.
        assert_eq!(find_match(LABELS, "bbb", Some(2), all), Some(1));
    }

    #[test]
    fn refining_keeps_the_current_item() {
        // "Bright" is current and still matches "br".
        assert_eq!(find_match(LABELS, "br", Some(2), all), Some(2));
    }

    #[test]
    fn skips_disabled_items() {
        assert_eq!(find_match(LABELS, "b", None, |i| i != 1), Some(2));
    }

    #[test]
    fn type_ahead_resets_after_a_pause() {
        let mut ahead = TypeAhead::default();
        let t0 = Instant::now();
        assert_eq!(ahead.push("C", t0), "c");
        assert_eq!(ahead.push("l", t0 + Duration::from_millis(300)), "cl");
        let later = t0 + Duration::from_millis(300) + TypeAhead::RESET_AFTER * 2;
        assert_eq!(ahead.push("d", later), "d");
    }
}
