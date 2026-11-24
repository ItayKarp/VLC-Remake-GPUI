//! Fills a translated text's `{name}` placeholders with values.

use std::fmt::{Display, Write};

/// `template` with each `{name}` replaced by the value named `name`. Braces that aren't a
/// placeholder with a value stay as they are, so a translation that drops or misspells one
/// shows rather than panics.
pub fn fill(template: &str, values: &[(&str, &dyn Display)]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let value = after.find('}').and_then(|close| {
            let name = &after[..close];
            values.iter().find(|(key, _)| *key == name).map(|(_, value)| (close, value))
        });
        if let Some((close, value)) = value {
            let _ = write!(out, "{value}");
            rest = &after[close + 1..];
        } else {
            out.push('{');
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// A number of things a plural form is picked for ([`trn!`]): any integer. A negative count
/// counts as its size.
pub trait Count {
    /// The count as `u64`, saturating.
    fn into_count(self) -> u64;
}

macro_rules! count_unsigned {
    ($($t:ty),*) => {$(
        impl Count for $t {
            fn into_count(self) -> u64 {
                u64::try_from(self).unwrap_or(u64::MAX)
            }
        }
    )*};
}

macro_rules! count_signed {
    ($($t:ty),*) => {$(
        impl Count for $t {
            fn into_count(self) -> u64 {
                u64::try_from(self.unsigned_abs()).unwrap_or(u64::MAX)
            }
        }
    )*};
}

count_unsigned!(u8, u16, u32, u64, usize);
count_signed!(i8, i16, i32, i64, isize);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_placeholders_are_filled_in_any_order() {
        let (a, b) = (3, "Movie.mkv");
        assert_eq!(fill("{b} is {a}", &[("a", &a), ("b", &b)]), "Movie.mkv is 3");
    }

    #[test]
    fn unknown_or_broken_placeholders_stay() {
        let a = 1;
        assert_eq!(fill("{a} {b} {", &[("a", &a)]), "1 {b} {");
        assert_eq!(fill("no braces", &[]), "no braces");
    }
}
