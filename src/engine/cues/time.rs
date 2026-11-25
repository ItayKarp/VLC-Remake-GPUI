//! Timestamps as subtitle formats write them: `01:02:03,456` (SRT), `01:02.345` (WebVTT),
//! `0:01:02.34` (ASS), read leniently: either decimal separator, any number of fraction digits,
//! and a negative time counting as zero.

use std::time::Duration;

const SECONDS_PER_MINUTE: u64 = 60;
const MINUTES_PER_HOUR: u64 = 60;
/// The most fraction digits that count: milliseconds.
const FRACTION_DIGITS: usize = 3;
const MILLIS_PER_SECOND: u64 = 1_000;

/// The time `text` gives as `[hours:]minutes:seconds[.fraction]`.
pub(super) fn parse(text: &str) -> Option<Duration> {
    let text = text.trim();
    // A negative time (a shifted file) starts at the beginning. One sign only: a run of them
    // isn't a time.
    match text.strip_prefix('-') {
        Some(positive) => unsigned(positive.trim_start()).map(|_| Duration::ZERO),
        None => unsigned(text),
    }
}

/// [`parse`] for a time without a sign.
fn unsigned(text: &str) -> Option<Duration> {
    let (whole, fraction) = match text.find([',', '.']) {
        Some(at) => (&text[..at], &text[at + 1..]),
        None => (text, ""),
    };
    let parts: Vec<u64> = whole.split(':').map(number).collect::<Option<_>>()?;
    let (hours, minutes, seconds) = match parts[..] {
        [minutes, seconds] => (0, minutes, seconds),
        [hours, minutes, seconds] => (hours, minutes, seconds),
        _ => return None,
    };
    // Checked: a file can write any number of hours.
    let seconds = hours
        .checked_mul(MINUTES_PER_HOUR)?
        .checked_add(minutes)?
        .checked_mul(SECONDS_PER_MINUTE)?
        .checked_add(seconds)?;
    Some(Duration::from_secs(seconds) + Duration::from_millis(millis(fraction)?))
}

/// The milliseconds a decimal fraction's digits give: `5` is 500, `45` is 450, `4567` is 456.
fn millis(fraction: &str) -> Option<u64> {
    if fraction.is_empty() {
        return Some(0);
    }
    let digits: String = fraction.chars().take(FRACTION_DIGITS).collect();
    let value = number(&digits)?;
    let scale = 10_u64.pow(u32::try_from(FRACTION_DIGITS - digits.len()).ok()?);
    Some((value * scale).min(MILLIS_PER_SECOND - 1))
}

/// A run of ASCII digits as a number.
fn number(text: &str) -> Option<u64> {
    let text = text.trim();
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(millis: u64) -> Duration {
        Duration::from_millis(millis)
    }

    #[test]
    fn every_format_s_timestamps_read() {
        assert_eq!(parse("01:02:03,456"), Some(ms(3_723_456)));
        assert_eq!(parse("01:02:03.456"), Some(ms(3_723_456)));
        assert_eq!(parse("02:03.456"), Some(ms(123_456)));
        assert_eq!(parse("0:00:01.50"), Some(ms(1_500)));
        assert_eq!(parse(" 00:00:01,5 "), Some(ms(1_500)));
        assert_eq!(parse("00:00:01,4567"), Some(ms(1_456)));
        assert_eq!(parse("00:00:02"), Some(ms(2_000)));
    }

    #[test]
    fn a_negative_time_is_zero_and_nonsense_is_nothing() {
        assert_eq!(parse("-00:00:01,000"), Some(ms(0)));
        assert_eq!(parse("1:2:3:4"), None);
        assert_eq!(parse("aa:bb:cc"), None);
        assert_eq!(parse(""), None);
    }

    #[test]
    fn hostile_timestamps_are_nothing_rather_than_a_crash() {
        // Found by fuzzing (S9.7.1): each sign was one more level of recursion.
        assert_eq!(parse(&format!("{}00:00:01,000", "-".repeat(1_000_000))), None);
        assert_eq!(parse("18446744073709551615:00:00"), None);
        assert_eq!(parse("00:18446744073709551615:00"), None);
    }
}
