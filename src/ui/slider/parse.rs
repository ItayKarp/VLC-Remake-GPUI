//! Reading the number a user typed into a slider's numeric field.

/// Parses a number typed into a slider's field: ignores a trailing unit
/// ("200%", "-1.5 dB", "0.000 s") and accepts a comma as the decimal separator.
pub fn parse_number(text: &str) -> Option<f64> {
    let text = text.trim();
    let end = text
        .char_indices()
        .take_while(|(i, c)| {
            c.is_ascii_digit()
                || matches!(c, '.' | ',')
                || (*i == 0 && matches!(c, '-' | '+' | '−'))
        })
        .map(|(i, c)| i + c.len_utf8())
        .last()?;
    let number = text[..end].replace(',', ".").replace('−', "-");
    number.parse::<f64>().ok().filter(|v| v.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_numbers_with_units() {
        assert_eq!(parse_number("200%"), Some(200.));
        assert_eq!(parse_number(" -1.5 dB"), Some(-1.5));
        assert_eq!(parse_number("0,25"), Some(0.25));
        assert_eq!(parse_number("−3"), Some(-3.));
        assert_eq!(parse_number("+2 s"), Some(2.));
        assert_eq!(parse_number("abc"), None);
        assert_eq!(parse_number(""), None);
        assert_eq!(parse_number("1e999"), Some(1.));
    }
}
