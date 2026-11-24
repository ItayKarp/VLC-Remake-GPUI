//! A small glob language for "Never remember items matching" (S5.5.1): `*` matches any run of
//! characters, `?` any one character, and `[abc]`, `[a-z]` or `[!abc]` one character of (or not
//! of) a set. Matching ignores case. A pattern is parsed whole or refused, so a typo never
//! applies as a shorter pattern.

/// A pattern that doesn't parse, with why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlobError(pub String);

/// One parsed glob pattern.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Glob {
    tokens: Vec<Token>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
    /// This character, lowercased.
    Literal(char),
    /// `?`: any one character.
    AnyChar,
    /// `*`: any run of characters, empty included.
    AnyRun,
    /// `[…]`: one character in (or, `negated`, not in) the ranges, lowercased.
    Set { negated: bool, ranges: Vec<(char, char)> },
}

impl Glob {
    /// Parses `pattern`; an unclosed or empty `[…]` is refused.
    pub fn parse(pattern: &str) -> Result<Self, GlobError> {
        let mut tokens = Vec::new();
        let mut chars = pattern.chars().peekable();
        while let Some(c) = chars.next() {
            tokens.push(match c {
                '*' => Token::AnyRun,
                '?' => Token::AnyChar,
                '[' => parse_set(&mut chars, pattern)?,
                c => Token::Literal(fold(c)),
            });
        }
        tokens.dedup_by(|a, b| *a == Token::AnyRun && *b == Token::AnyRun);
        Ok(Self { tokens })
    }

    /// Whether the whole of `text` matches, ignoring case.
    pub fn matches(&self, text: &str) -> bool {
        let text: Vec<char> = text.chars().map(fold).collect();
        matches_from(&self.tokens, &text)
    }
}

/// Reads a `[…]` set after its `[`.
fn parse_set(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    pattern: &str,
) -> Result<Token, GlobError> {
    let unclosed = || GlobError(tr!("\"{pattern}\" has an unfinished [ ] set", pattern));
    let negated = chars.next_if(|c| *c == '!' || *c == '^').is_some();
    let mut ranges = Vec::new();
    loop {
        let c = chars.next().ok_or_else(unclosed)?;
        // A ] first in the set is a character, as in shells.
        if c == ']' && !ranges.is_empty() {
            return Ok(Token::Set { negated, ranges });
        }
        let end = match chars.next_if_eq(&'-') {
            Some(_) => chars.next().filter(|end| *end != ']').ok_or_else(unclosed)?,
            None => c,
        };
        ranges.push((fold(c), fold(end)));
    }
}

/// Backtracks only to the last `*`, which keeps matching linear in practice.
fn matches_from(tokens: &[Token], text: &[char]) -> bool {
    let (mut t, mut i) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while i < text.len() {
        match tokens.get(t) {
            Some(Token::AnyRun) => {
                star = Some((t, i));
                t += 1;
            }
            Some(token) if token.accepts(text[i]) => {
                t += 1;
                i += 1;
            }
            _ => {
                let Some((star_t, star_i)) = star else { return false };
                star = Some((star_t, star_i + 1));
                t = star_t + 1;
                i = star_i + 1;
            }
        }
    }
    tokens[t..].iter().all(|token| *token == Token::AnyRun)
}

impl Token {
    fn accepts(&self, c: char) -> bool {
        match self {
            Self::Literal(literal) => *literal == c,
            Self::AnyChar => true,
            Self::AnyRun => false,
            Self::Set { negated, ranges } => {
                ranges.iter().any(|(low, high)| (*low..=*high).contains(&c)) != *negated
            }
        }
    }
}

/// `c` as matching compares it.
fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glob(pattern: &str) -> Glob {
        Glob::parse(pattern).expect("a valid pattern")
    }

    #[test]
    fn stars_and_question_marks() {
        assert!(glob("*.part").matches("Film.PART"));
        assert!(!glob("*.part").matches("film.mkv"));
        assert!(glob("*secret*").matches("my secret clip.mp4"));
        assert!(glob("ep??.mkv").matches("ep01.mkv"));
        assert!(!glob("ep??.mkv").matches("ep1.mkv"));
        assert!(glob("**").matches(""));
        assert!(glob("a*b*c").matches("aXXbYYbZc"));
        assert!(!glob("a*b*c").matches("aXXbYY"));
    }

    #[test]
    fn sets() {
        assert!(glob("[abc].mp3").matches("B.mp3"));
        assert!(glob("ep[0-9].mkv").matches("ep7.mkv"));
        assert!(!glob("ep[!0-9].mkv").matches("ep7.mkv"));
        assert!(glob("[]x]").matches("]"));
    }

    #[test]
    fn broken_sets_are_refused_whole() {
        assert!(Glob::parse("ep[0-9.mkv").is_err());
        assert!(Glob::parse("[").is_err());
        assert!(Glob::parse("[a-]").is_err());
    }
}
