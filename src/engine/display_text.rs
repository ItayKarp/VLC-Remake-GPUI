//! Text that comes from media made safe to show (S9.7.2): titles, artists and albums from tags,
//! chapter and track names, and a stream's own name are written by whoever made the file or
//! runs the server, so they are one line, without control characters or the bidirectional
//! overrides that make `exe.mp4` look like `4pm.exe`, and of a sane length.

/// The longest text kept, in characters; past it the text ends in an ellipsis.
pub const MAX_CHARS: usize = 512;
/// What ends a text cut at [`MAX_CHARS`].
const ELLIPSIS: char = '…';

/// `text` as one line: control characters and line breaks become spaces, runs of spaces one,
/// the bidirectional embeddings, overrides and isolates (and a byte order mark) are dropped,
/// and it is cut at [`MAX_CHARS`]. `None` when nothing is left.
pub fn clean(text: &str) -> Option<String> {
    let mut line = String::with_capacity(text.len().min(MAX_CHARS * 4));
    let mut chars = 0;
    let mut pending_space = false;
    for c in text.chars().filter(|&c| !is_hidden_control(c)) {
        if c.is_whitespace() || c.is_control() {
            pending_space = !line.is_empty();
            continue;
        }
        if chars + usize::from(pending_space) >= MAX_CHARS {
            line.push(ELLIPSIS);
            return Some(line);
        }
        if pending_space {
            line.push(' ');
            chars += 1;
            pending_space = false;
        }
        line.push(c);
        chars += 1;
    }
    (!line.is_empty()).then_some(line)
}

/// [`clean`] for an optional tag, as libVLC hands them over.
pub fn clean_tag(text: Option<String>) -> Option<String> {
    text.and_then(|text| clean(&text))
}

/// Whether `c` changes how the text around it is ordered or read without showing anything:
/// the bidirectional embeddings, overrides and isolates, and a byte order mark.
fn is_hidden_control(c: char) -> bool {
    matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{feff}')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn makes_one_plain_line() {
        assert_eq!(clean("  Song\r\n\tName\u{7}  ").as_deref(), Some("Song Name"));
        assert_eq!(clean("a\u{85}b\u{2028}c").as_deref(), Some("a b c"));
        assert_eq!(clean(" \n\u{0}\t"), None);
        assert_eq!(clean_tag(Some("שלום עולם".into())).as_deref(), Some("שלום עולם"));
    }

    #[test]
    fn drops_bidirectional_overrides() {
        assert_eq!(clean("Holiday\u{202e}4pm.exe").as_deref(), Some("Holiday4pm.exe"));
        assert_eq!(clean("\u{feff}\u{2067}x\u{2069}").as_deref(), Some("x"));
    }

    #[test]
    fn long_text_is_cut_with_an_ellipsis() {
        let cut = clean(&"é".repeat(MAX_CHARS * 3)).unwrap();
        assert_eq!(cut.chars().count(), MAX_CHARS + 1);
        assert!(cut.ends_with(ELLIPSIS));
        assert_eq!(clean(&"x".repeat(MAX_CHARS)).unwrap().chars().count(), MAX_CHARS);
    }
}
