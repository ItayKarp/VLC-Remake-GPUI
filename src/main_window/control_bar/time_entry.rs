//! What can be typed into the current time to jump (S2.4.2): `hh:mm:ss`, `mm:ss`, plain
//! seconds, `1h2m3s`, any of those after `+` or `-` for a jump from where playback is, and a
//! percentage of the length (`50%`).
//!
//! Seconds may have a fraction (`1:02.5`). Times past the end are left for the player to clamp
//! to the end, and a jump back past the start stops at the start. Only ASCII digits count,
//! whatever the locale's own digits are.

use std::time::Duration;

use crate::engine::values::{Offset, SeekTarget};

/// Typed text that isn't a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Invalid;

/// Seconds in a minute and minutes in an hour.
const SIXTY: f64 = 60.;
/// The longest time accepted, so the conversions below can't overflow: 100 000 hours.
const MAX_SECONDS: f64 = 360_000_000.;

/// Where `text` asks to jump.
pub fn parse(text: &str) -> Result<SeekTarget, Invalid> {
    let text = text.trim();
    if let Some(percent) = text.strip_suffix('%') {
        let percent = number(percent.trim_end())?;
        return Ok(SeekTarget::Fraction((percent / 100.).min(1.) as f32));
    }
    if let Some(rest) = text.strip_prefix('+') {
        return Ok(SeekTarget::Relative(offset(time(rest.trim_start())?, 1)));
    }
    if let Some(rest) = text.strip_prefix('-') {
        return Ok(SeekTarget::Relative(offset(time(rest.trim_start())?, -1)));
    }
    Ok(SeekTarget::Absolute(time(text)?))
}

/// A time in any of the absolute forms.
fn time(text: &str) -> Result<Duration, Invalid> {
    let seconds = if text.contains(':') {
        clock(text)?
    } else if text.ends_with(['h', 'm', 's', 'H', 'M', 'S']) {
        units(&text.to_ascii_lowercase())?
    } else {
        number(text)?
    };
    if seconds > MAX_SECONDS {
        return Err(Invalid);
    }
    Ok(Duration::from_secs_f64(seconds))
}

/// `hh:mm:ss` or `mm:ss`. The first field may be any size; the ones after it stay under 60.
fn clock(text: &str) -> Result<f64, Invalid> {
    let fields: Vec<&str> = text.split(':').collect();
    if fields.len() > 3 {
        return Err(Invalid);
    }
    let (last, whole) = fields.split_last().ok_or(Invalid)?;
    let mut seconds = 0.;
    for (index, field) in whole.iter().enumerate() {
        let value = integer(field)?;
        if index > 0 && value >= SIXTY {
            return Err(Invalid);
        }
        seconds = seconds * SIXTY + value;
    }
    let last = number(last)?;
    if last >= SIXTY {
        return Err(Invalid);
    }
    Ok(seconds * SIXTY + last)
}

/// `1h2m3s` and any part of it (`90s`, `1h30m`), the units in that order, each at most once.
fn units(text: &str) -> Result<f64, Invalid> {
    let mut seconds = 0.;
    let mut rest = text;
    let mut units: &[(char, f64)] = &[('h', SIXTY * SIXTY), ('m', SIXTY), ('s', 1.)];
    while !rest.is_empty() {
        let end = rest.find(|c: char| c.is_ascii_alphabetic()).ok_or(Invalid)?;
        let unit = rest[end..].chars().next().ok_or(Invalid)?;
        let position = units.iter().position(|(name, _)| *name == unit).ok_or(Invalid)?;
        let value = if unit == 's' { number(&rest[..end])? } else { integer(&rest[..end])? };
        seconds += value * units[position].1;
        units = &units[position + 1..];
        rest = rest[end + 1..].trim_start();
    }
    Ok(seconds)
}

/// ASCII digits with an optional fraction, nothing else: no sign, exponent or `inf`.
fn number(text: &str) -> Result<f64, Invalid> {
    let text = text.trim();
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    let digits = |part: &str| part.bytes().all(|b| b.is_ascii_digit());
    if (whole.is_empty() && fraction.is_empty()) || !digits(whole) || !digits(fraction) {
        return Err(Invalid);
    }
    text.parse().map_err(|_| Invalid)
}

/// ASCII digits alone.
fn integer(text: &str) -> Result<f64, Invalid> {
    let text = text.trim();
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Invalid);
    }
    text.parse().map_err(|_| Invalid)
}

/// `time` as an offset `sign` way.
fn offset(time: Duration, sign: i64) -> Offset {
    let millis = i64::try_from(time.as_millis()).unwrap_or(i64::MAX);
    Offset::from_millis(sign * millis)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn absolute(text: &str) -> Duration {
        match parse(text) {
            Ok(SeekTarget::Absolute(time)) => time,
            other => panic!("{text:?} gave {other:?}"),
        }
    }

    #[test]
    fn reads_clock_times() {
        assert_eq!(absolute("00:02:10"), Duration::from_secs(130));
        assert_eq!(absolute("1:02:03"), Duration::from_secs(3723));
        assert_eq!(absolute("2:10"), Duration::from_secs(130));
        assert_eq!(absolute("90:00"), Duration::from_mins(90));
        assert_eq!(absolute("100:00:01"), Duration::from_secs(360_001));
        assert_eq!(absolute(" 0:01.5 "), Duration::from_millis(1500));
    }

    #[test]
    fn reads_plain_seconds_and_units() {
        assert_eq!(absolute("90"), Duration::from_secs(90));
        assert_eq!(absolute("1h2m3s"), Duration::from_secs(3723));
        assert_eq!(absolute("1H 30M"), Duration::from_mins(90));
        assert_eq!(absolute("45s"), Duration::from_secs(45));
        assert_eq!(absolute("2m"), Duration::from_secs(120));
    }

    #[test]
    fn reads_relative_jumps() {
        assert_eq!(parse("+30"), Ok(SeekTarget::Relative(Offset::from_millis(30_000))));
        assert_eq!(parse("-30"), Ok(SeekTarget::Relative(Offset::from_millis(-30_000))));
        assert_eq!(parse("-1:00"), Ok(SeekTarget::Relative(Offset::from_millis(-60_000))));
        assert_eq!(parse("+2m"), Ok(SeekTarget::Relative(Offset::from_millis(120_000))));
    }

    #[test]
    fn reads_percentages_up_to_the_end() {
        assert_eq!(parse("50%"), Ok(SeekTarget::Fraction(0.5)));
        assert_eq!(parse("12.5 %"), Ok(SeekTarget::Fraction(0.125)));
        assert_eq!(parse("250%"), Ok(SeekTarget::Fraction(1.)));
    }

    #[test]
    fn rejects_what_isnt_a_time() {
        for text in [
            "",
            " ",
            "abc",
            "1:2:3:4",
            "1:60",
            "1:60:00",
            "::",
            "1:",
            ":30",
            "--30",
            "+",
            "1s2m",
            "1h1h",
            "1x",
            "-50%",
            "%",
            "1e3",
            "inf",
            "NaN",
            "٣٠",
            "1.5h",
            "99999999999",
        ] {
            assert_eq!(parse(text), Err(Invalid), "{text:?}");
        }
    }
}
