//! The dates and lengths podcast feeds write: RSS's RFC 2822 `pubDate` ("Tue, 10 Jun 2025
//! 04:00:00 GMT"), Atom's RFC 3339 `published` ("2025-06-10T04:00:00Z"), and iTunes'
//! `duration` ("1:02:03", "62:03" or "3723"). Feeds get them wrong often enough that anything
//! unreadable is `None` rather than an error.

/// Seconds in a minute, an hour and a day.
const MINUTE: i64 = 60;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;
/// RFC 2822's month names, in order.
const MONTHS: [&str; 12] =
    ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

/// An RFC 2822 date as seconds since the Unix epoch. The weekday is optional and ignored; the
/// zone is an offset or a name RFC 822 knows.
pub fn rfc2822(text: &str) -> Option<i64> {
    let text = text.split_once(',').map_or(text, |(_, rest)| rest);
    let mut words = text.split_whitespace();
    let day: u32 = words.next()?.parse().ok()?;
    let month = words.next()?.get(..3)?.to_ascii_lowercase();
    let month = MONTHS.iter().position(|name| *name == month)? as u32 + 1;
    let year = full_year(words.next()?.parse().ok()?);
    let time = clock(words.next()?)?;
    let offset = words.next().map_or(Some(0), zone)?;
    Some(at(year, month, day)? + time - offset)
}

/// An RFC 3339 date-time (or a bare date) as seconds since the Unix epoch.
pub fn rfc3339(text: &str) -> Option<i64> {
    let text = text.trim();
    let (date, rest) = text.split_at_checked(10)?;
    let mut parts = date.split('-');
    let year = parts.next()?.parse().ok()?;
    let month = parts.next()?.parse().ok()?;
    let day = parts.next()?.parse().ok()?;
    let midnight = at(year, month, day)?;
    let Some(rest) = rest.strip_prefix(['T', 't', ' ']) else {
        return rest.is_empty().then_some(midnight);
    };
    let zone_at = rest.find(['Z', 'z', '+', '-']).unwrap_or(rest.len());
    let (time, zone_text) = rest.split_at(zone_at);
    let time = time.split('.').next()?;
    let offset = if zone_text.is_empty() { 0 } else { zone(zone_text)? };
    Some(midnight + clock(time)? - offset)
}

/// An iTunes duration as whole seconds: `h:mm:ss`, `mm:ss` or seconds, with any fraction
/// dropped.
pub fn duration(text: &str) -> Option<u32> {
    let text = text.trim();
    let whole = text.split('.').next()?;
    let mut seconds: u32 = 0;
    for part in whole.split(':') {
        let part: u32 = part.trim().parse().ok()?;
        seconds = seconds.checked_mul(60)?.checked_add(part)?;
    }
    (seconds > 0).then_some(seconds)
}

/// `hh:mm` or `hh:mm:ss` as seconds into the day.
fn clock(text: &str) -> Option<i64> {
    let mut parts = text.split(':').map(|part| part.parse::<i64>().ok());
    let hours = parts.next()??;
    let minutes = parts.next()??;
    let seconds = parts.next().unwrap_or(Some(0))?;
    let valid =
        (0..24).contains(&hours) && (0..60).contains(&minutes) && (0..61).contains(&seconds);
    valid.then_some(hours * HOUR + minutes * MINUTE + seconds)
}

/// A zone as seconds east of UTC: `Z`, `+hhmm`, `+hh:mm`, or an RFC 822 name.
fn zone(text: &str) -> Option<i64> {
    let named = match text.to_ascii_uppercase().as_str() {
        "Z" | "UT" | "UTC" | "GMT" => Some(0),
        "EDT" => Some(-4),
        "EST" | "CDT" => Some(-5),
        "CST" | "MDT" => Some(-6),
        "MST" | "PDT" => Some(-7),
        "PST" => Some(-8),
        _ => None,
    };
    if let Some(hours) = named {
        return Some(hours * HOUR);
    }
    let sign = match text.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let digits: String = text[1..].chars().filter(char::is_ascii_digit).collect();
    if digits.len() != 4 {
        return None;
    }
    let hours: i64 = digits[..2].parse().ok()?;
    let minutes: i64 = digits[2..].parse().ok()?;
    Some(sign * (hours * HOUR + minutes * MINUTE))
}

/// A two-digit year as RFC 2822 reads it.
fn full_year(year: i64) -> i64 {
    match year {
        0..50 => 2000 + year,
        50..100 => 1900 + year,
        _ => year,
    }
}

/// Midnight UTC of a date, as seconds since the Unix epoch (Howard Hinnant's days-from-civil).
fn at(year: i64, month: u32, day: u32) -> Option<i64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year.rem_euclid(400);
    let shifted_month = i64::from((month + 9) % 12);
    let day_of_year = (153 * shifted_month + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some((era * 146_097 + day_of_era - 719_468) * DAY)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2025-06-10T04:00:00Z.
    const JUNE_10: i64 = 1_749_528_000;

    #[test]
    fn rss_dates_read_with_or_without_a_weekday_and_in_any_zone() {
        assert_eq!(rfc2822("Tue, 10 Jun 2025 04:00:00 GMT"), Some(JUNE_10));
        assert_eq!(rfc2822("10 June 2025 06:00:00 +0200"), Some(JUNE_10));
        assert_eq!(rfc2822("Mon, 09 Jun 2025 21:00 PDT"), Some(JUNE_10));
        assert_eq!(rfc2822("Tue, 10 Jun 25 04:00:00 Z"), Some(JUNE_10));
        assert_eq!(rfc2822("yesterday"), None);
        assert_eq!(rfc2822("Tue, 10 Foo 2025 04:00:00 GMT"), None);
    }

    #[test]
    fn atom_dates_read_with_fractions_and_offsets() {
        assert_eq!(rfc3339("2025-06-10T04:00:00Z"), Some(JUNE_10));
        assert_eq!(rfc3339("2025-06-10T00:00:00.250-04:00"), Some(JUNE_10));
        assert_eq!(rfc3339("2025-06-10"), Some(JUNE_10 - 4 * HOUR));
        assert_eq!(rfc3339("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(rfc3339("2025-13-10T04:00:00Z"), None);
    }

    #[test]
    fn durations_read_as_clocks_or_seconds() {
        assert_eq!(duration("1:02:03"), Some(3723));
        assert_eq!(duration("62:03"), Some(3723));
        assert_eq!(duration("3723"), Some(3723));
        assert_eq!(duration("3723.6"), Some(3723));
        assert_eq!(duration("0"), None);
        assert_eq!(duration("an hour"), None);
    }
}
