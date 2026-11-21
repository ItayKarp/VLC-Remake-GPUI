//! The bar's time readouts: `hh:mm:ss` as designed (`00:02:10`), `hhh:mm:ss` past 99 hours
//! (S2.4.4), the time left as `-00:23:30` (S2.4.3), and `--:--:--` while the length is unknown.
//! Digits are always ASCII and drawn tabular, so the bar doesn't jitter as they change.

use std::sync::Arc;
use std::time::Duration;

use gpui::FontFeatures;

/// What a readout shows for a time that isn't known (a live stream's length).
pub const UNKNOWN: &str = "--:--:--";

/// `time` as `hh:mm:ss`, whole seconds rounded down.
pub fn format(time: Duration) -> String {
    let seconds = time.as_secs();
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

/// The time left after `position`, as `-hh:mm:ss`. Whole seconds are taken from both, so the
/// current time and the time left always add up to the length.
pub fn format_remaining(position: Duration, duration: Duration) -> String {
    let left = duration.as_secs().saturating_sub(position.as_secs());
    format!("-{}", format(Duration::from_secs(left)))
}

/// Font features that give every digit the same width (`tnum`).
pub fn tabular_digits() -> FontFeatures {
    FontFeatures(Arc::new(vec![("tnum".into(), 1)]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_as_designed() {
        assert_eq!(format(Duration::from_secs(130)), "00:02:10");
        assert_eq!(format(Duration::from_secs(25 * 60 + 40)), "00:25:40");
        assert_eq!(format(Duration::from_millis(999)), "00:00:00");
    }

    #[test]
    fn grows_past_99_hours() {
        assert_eq!(format(Duration::from_secs(100 * 3600 + 1)), "100:00:01");
    }

    #[test]
    fn remaining_time_counts_down_to_zero() {
        let length = Duration::from_secs(25 * 60 + 40);
        assert_eq!(format_remaining(Duration::from_millis(130_900), length), "-00:23:30");
        assert_eq!(format_remaining(length + Duration::from_secs(5), length), "-00:00:00");
    }
}
