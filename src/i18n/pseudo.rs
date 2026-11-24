//! The pseudo-locales' made-up translations: English that still reads, but looks and runs like
//! a translation, so what doesn't fit, what isn't translated and what doesn't mirror shows.
//! Placeholders (`{title}`) are kept as they are, since the code fills them in.

/// How much longer the accented pseudo-locale makes text, as padding per letter (2 in 5, 40%):
/// German and Finnish run 30–40% longer than English (S9.2.3).
const EXPANSION: (usize, usize) = (2, 5);
/// What pads accented text to its longer length.
const PAD: char = '·';
/// Makes the paragraph right to left: it aligns to the right, and punctuation at its ends goes
/// to the right-to-left side.
const RIGHT_TO_LEFT_MARK: char = '\u{200F}';

/// `text` accented, bracketed and about 40% longer: "Play" is "[Ƥļåŷ··]".
pub fn accented(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2 + 2);
    out.push('[');
    let mut letters = 0;
    for piece in pieces(text) {
        match piece {
            Piece::Placeholder(placeholder) => out.push_str(placeholder),
            Piece::Text(plain) => {
                letters += plain.chars().filter(char::is_ascii_alphabetic).count();
                out.extend(plain.chars().map(accent));
            }
        }
    }
    let padding = (letters * EXPANSION.0).div_ceil(EXPANSION.1);
    out.extend(std::iter::repeat_n(PAD, padding));
    out.push(']');
    out
}

/// `text` as a right-to-left paragraph whose English words still read forwards, so the
/// mirrored layout can be checked with readable labels.
pub fn right_to_left(text: &str) -> String {
    format!("{RIGHT_TO_LEFT_MARK}{text}{RIGHT_TO_LEFT_MARK}")
}

/// A run of `text`: plain text, or a `{placeholder}` the code fills in.
enum Piece<'a> {
    Text(&'a str),
    Placeholder(&'a str),
}

/// `text` split into plain runs and `{name}` placeholders.
fn pieces(text: &str) -> Vec<Piece<'_>> {
    let mut pieces = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        let Some(length) = rest[open..].find('}').map(|close| close + 1) else { break };
        let candidate = &rest[open..open + length];
        let is_placeholder =
            candidate[1..length - 1].chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        let split = if is_placeholder { open } else { open + 1 };
        if split > 0 {
            pieces.push(Piece::Text(&rest[..split]));
        }
        if is_placeholder {
            pieces.push(Piece::Placeholder(candidate));
            rest = &rest[open + length..];
        } else {
            rest = &rest[split..];
        }
    }
    if !rest.is_empty() {
        pieces.push(Piece::Text(rest));
    }
    pieces
}

/// An accented look-alike of an ASCII letter; other characters stay.
fn accent(c: char) -> char {
    const LOWER: [char; 26] = [
        'å', 'ƀ', 'ç', 'ð', 'é', 'ƒ', 'ĝ', 'ĥ', 'î', 'ĵ', 'ķ', 'ļ', 'ɱ', 'ñ', 'ö', 'þ', 'ǫ', 'ŕ',
        'š', 'ţ', 'û', 'ṽ', 'ŵ', 'ẋ', 'ý', 'ž',
    ];
    const UPPER: [char; 26] = [
        'Å', 'Ɓ', 'Ç', 'Ð', 'É', 'Ƒ', 'Ĝ', 'Ĥ', 'Î', 'Ĵ', 'Ķ', 'Ļ', 'Ṁ', 'Ñ', 'Ö', 'Ƥ', 'Ǫ', 'Ŕ',
        'Š', 'Ţ', 'Û', 'Ṽ', 'Ŵ', 'Ẋ', 'Ý', 'Ž',
    ];
    match c {
        'a'..='z' => LOWER[(c as u8 - b'a') as usize],
        'A'..='Z' => UPPER[(c as u8 - b'A') as usize],
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accented_text_is_longer_bracketed_and_keeps_placeholders() {
        assert_eq!(accented("Play"), "[Ƥļåý··]");
        assert_eq!(accented("Playing {title}"), "[Ƥļåýîñĝ {title}···]");
        assert_eq!(accented("{count} items"), "[{count} îţéɱš··]");
        assert_eq!(accented("Not {a placeholder}"), "[Ñöţ {å þļåçéĥöļðéŕ}······]");
    }

    #[test]
    fn right_to_left_text_keeps_its_words_in_a_right_to_left_paragraph() {
        assert_eq!(right_to_left("Play {title} now!"), "\u{200F}Play {title} now!\u{200F}");
    }
}
