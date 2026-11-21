//! What a playlist's title may be (S3.5 edge cases): trimmed, with at least one visible
//! character, at most [`MAX_CHARS`] characters, and never the same as another playlist's.
//!
//! Decided: a title that another playlist already has is saved with a number after it
//! ("Mix (2)"), as file managers do with copies, rather than warned about. Two playlists can
//! then always be told apart in a menu or a picker.

/// The longest title kept, in characters.
pub const MAX_CHARS: usize = 100;

/// `raw` as a title: trimmed and cut to [`MAX_CHARS`], or `None` when nothing in it shows.
pub fn clean(raw: &str) -> Option<String> {
    let trimmed = raw.trim_matches(is_blank);
    if trimmed.is_empty() {
        return None;
    }
    let cut: String = trimmed.chars().take(MAX_CHARS).collect();
    Some(cut.trim_end_matches(is_blank).to_owned())
}

/// Whether `raw` would make a title, so Save can be enabled.
pub fn is_valid(raw: &str) -> bool {
    clean(raw).is_some()
}

/// `wanted`, or `wanted` with the lowest free number after it, so that no title in `taken`
/// matches it ignoring case. A wanted title that already ends in a number ("Mix (2)") counts on
/// from it rather than adding a second one.
pub fn unique<'a>(wanted: &str, taken: impl IntoIterator<Item = &'a str>) -> String {
    let taken: Vec<String> = taken.into_iter().map(str::to_lowercase).collect();
    let is_free = |title: &str| !taken.contains(&title.to_lowercase());
    if is_free(wanted) {
        return wanted.to_owned();
    }
    let (base, from) = numbered(wanted).unwrap_or((wanted, 1));
    // Each taken title rules out at most one number, so one more number than that is enough.
    (from + 1..)
        .take(taken.len() + 1)
        .map(|n| format!("{base} ({n})"))
        .find(|title| is_free(title))
        .expect("one of the first taken.len() + 1 numbers is free")
}

/// `"Mix (2)"` as `("Mix", 2)`.
fn numbered(title: &str) -> Option<(&str, u32)> {
    let (base, rest) = title.strip_suffix(')')?.rsplit_once(" (")?;
    // Six digits keep counting on from it far from overflowing.
    let digits = rest.len() <= 6 && rest.bytes().all(|b| b.is_ascii_digit());
    let n = rest.parse().ok().filter(|_| digits)?;
    (!base.is_empty()).then_some((base, n))
}

/// Characters that show nothing: spaces, controls and zero-width marks.
fn is_blank(c: char) -> bool {
    c.is_whitespace() || c.is_control() || matches!(c, '\u{200B}'..='\u{200D}' | '\u{FEFF}')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_are_trimmed_and_need_something_visible() {
        assert_eq!(clean("  Road trip \t").as_deref(), Some("Road trip"));
        assert_eq!(clean(""), None);
        assert_eq!(clean("   "), None);
        assert_eq!(clean("\u{200B}\u{FEFF} "), None);
        assert!(is_valid(" a "));
    }

    #[test]
    fn long_titles_are_cut() {
        let long = "é".repeat(500);
        assert_eq!(clean(&long).map(|t| t.chars().count()), Some(MAX_CHARS));
        let spaced = format!("{} tail", "a".repeat(MAX_CHARS - 1));
        assert_eq!(clean(&spaced), Some("a".repeat(MAX_CHARS - 1)), "no space left at the end");
    }

    #[test]
    fn a_taken_title_gets_the_next_free_number() {
        assert_eq!(unique("Mix", ["Other"]), "Mix");
        assert_eq!(unique("Mix", ["mix"]), "Mix (2)");
        assert_eq!(unique("Mix", ["Mix", "Mix (2)"]), "Mix (3)");
        assert_eq!(unique("Mix (2)", ["Mix", "Mix (2)"]), "Mix (3)");
        assert_eq!(unique("(2)", ["(2)"]), "(2) (2)");
        assert_eq!(unique("Mix (x)", ["Mix (x)"]), "Mix (x) (2)");
        assert_eq!(unique("Mix (4294967295)", ["Mix (4294967295)"]), "Mix (4294967295) (2)");
    }
}
