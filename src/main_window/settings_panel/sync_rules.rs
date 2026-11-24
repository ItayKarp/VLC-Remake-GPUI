//! How the Synchronization rows read and write their values (S6.3): a delay in seconds to the
//! millisecond (`-4.000 s`), typed as `-4,5`, `-4.5s` or `4500ms`; a subtitle speed as a factor
//! (`1.043×`) or the two frame rates it converts between (`25/23.976`); and a duration factor
//! (`Off`, `1.500×`).

use crate::engine::subtitle_sync::{MAX_DELAY_MS, MAX_DURATION_FACTOR, MAX_SPEED, MIN_SPEED};

/// Milliseconds in a second.
const MS_PER_SECOND: f64 = 1_000.;

/// A delay in seconds as the field shows it: `-4.000 s`.
pub(super) fn delay_text(seconds: f64) -> String {
    format!("{} s", crate::i18n::decimal(seconds, 3))
}

/// The delay slider's labels: `-10 s`, `0 s`, `+10 s`.
pub(super) fn delay_mark(seconds: f64) -> String {
    if seconds > 0. { format!("+{seconds} s") } else { format!("{seconds} s") }
}

/// A typed delay in seconds: `-4.5`, `-4,5`, `-4.5 s`, `−4.5s` or `4500ms`, up to ten minutes
/// either way.
pub(super) fn parse_delay(text: &str) -> Result<f64, &'static str> {
    let not_a_delay = tr!("A delay such as -4.5 s or 4500 ms");
    let (number, unit) = split(text).ok_or(not_a_delay)?;
    let seconds = match unit.as_str() {
        "" | "s" | "sec" | "secs" => number,
        "ms" => number / MS_PER_SECOND,
        _ => return Err(not_a_delay),
    };
    let limit = MAX_DELAY_MS as f64 / MS_PER_SECOND;
    if seconds.abs() > limit {
        return Err(tr!("Up to 10 minutes either way"));
    }
    Ok(seconds)
}

/// A subtitle speed as the field shows it: `1.043×`.
pub(super) fn speed_text(speed: f64) -> String {
    format!("{}×", crate::i18n::decimal(speed, 3))
}

/// A typed subtitle speed: `1.043`, `1.043x`, or two frame rates `25/23.976` (the video's over
/// the one the file was made for).
pub(super) fn parse_speed(text: &str) -> Result<f64, &'static str> {
    let not_a_speed = tr!("A speed such as 1.043 or 25/23.976");
    let speed = match text.split_once('/') {
        Some((video, file)) => {
            let rate =
                |text: &str| split(text).filter(|(_, unit)| unit.is_empty() || unit == "fps");
            let (video, file) =
                (rate(video).ok_or(not_a_speed)?.0, rate(file).ok_or(not_a_speed)?.0);
            if file <= 0. {
                return Err(not_a_speed);
            }
            video / file
        }
        None => factor(text).ok_or(not_a_speed)?,
    };
    if !(f64::from(MIN_SPEED)..=f64::from(MAX_SPEED)).contains(&speed) {
        return Err(tr!("Between 0.5× and 2×"));
    }
    Ok(speed)
}

/// A duration factor as the field shows it: `Off` or `1.500×`.
pub(super) fn duration_text(factor: f64) -> String {
    if factor <= 0. {
        tr!("Off").to_owned()
    } else {
        format!("{}×", crate::i18n::decimal(factor, 3))
    }
}

/// A typed duration factor: `off`, `0`, `1.5` or `1.5x`, up to 20.
pub(super) fn parse_duration(text: &str) -> Result<f64, &'static str> {
    let typed = text.trim();
    if typed.eq_ignore_ascii_case("off") || typed == tr!("Off") {
        return Ok(0.);
    }
    let factor = factor(text).ok_or(tr!("A factor such as 1.5, or Off"))?;
    if !(0. ..=f64::from(MAX_DURATION_FACTOR)).contains(&factor) {
        return Err(tr!("Between 0 (off) and 20"));
    }
    Ok(factor)
}

/// A number optionally followed by `x` or `×`.
fn factor(text: &str) -> Option<f64> {
    let (number, unit) = split(text)?;
    matches!(unit.as_str(), "" | "x" | "×").then_some(number)
}

/// Splits typed text into its number (a comma or `.` for the decimals, `-`, `+` or `−` before
/// it) and the unit after it, lowercased without spaces.
fn split(text: &str) -> Option<(f64, String)> {
    let text = text.trim();
    let end = text
        .char_indices()
        .find(|&(index, c)| {
            !(c.is_ascii_digit() || matches!(c, '.' | ',') || (index == 0 && "-+−".contains(c)))
        })
        .map_or(text.len(), |(index, _)| index);
    let number = text[..end].replace(',', ".").replace('−', "-");
    let number: f64 = number.parse().ok().filter(|value: &f64| value.is_finite())?;
    let unit: String = text[end..].chars().filter(|c| !c.is_whitespace()).collect();
    Some((number, unit.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delays_read_in_seconds_or_milliseconds() {
        assert_eq!(parse_delay("-4,5"), Ok(-4.5));
        assert_eq!(parse_delay("-4.5s"), Ok(-4.5));
        assert_eq!(parse_delay("4500ms"), Ok(4.5));
        assert_eq!(parse_delay(" −0.25 s "), Ok(-0.25));
        assert_eq!(parse_delay("-30"), Ok(-30.));
        assert!(parse_delay("-601").is_err());
        assert!(parse_delay("4 min").is_err());
        assert!(parse_delay("soon").is_err());
        assert_eq!(delay_text(-4.), "-4.000 s");
        assert_eq!(delay_mark(10.), "+10 s");
        assert_eq!(delay_mark(0.), "0 s");
    }

    #[test]
    fn speeds_read_as_factors_or_frame_rates() {
        assert_eq!(parse_speed("1.5x"), Ok(1.5));
        let pal = parse_speed("25/23.976").expect("a speed");
        assert!((pal - 1.0427).abs() < 1e-4);
        assert!(parse_speed("25 fps / 23.976 fps").is_ok());
        assert!(parse_speed("3").is_err());
        assert!(parse_speed("25/0").is_err());
        assert_eq!(speed_text(1.), "1.000×");
    }

    #[test]
    fn duration_factors_read_off_and_numbers() {
        assert_eq!(parse_duration("Off"), Ok(0.));
        assert_eq!(parse_duration("1,5×"), Ok(1.5));
        assert!(parse_duration("21").is_err());
        assert_eq!(duration_text(0.), "Off");
        assert_eq!(duration_text(2.), "2.000×");
    }
}
