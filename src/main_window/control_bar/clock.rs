//! The bar's time readouts: `hh:mm:ss` as designed (`00:02:10`), `hhh:mm:ss` past 99 hours
//! (S2.4.4), and `--:--:--` while the length is unknown.

use std::time::Duration;

/// What a readout shows for a time that isn't known (a live stream's length).
pub const UNKNOWN: &str = "--:--:--";

/// `time` as `hh:mm:ss`, whole seconds rounded down.
pub fn format(time: Duration) -> String {
    let seconds = time.as_secs();
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

/// `time` as `hh:mm:ss`, or [`UNKNOWN`].
pub fn format_or_unknown(time: Option<Duration>) -> String {
    time.map_or_else(|| UNKNOWN.to_owned(), format)
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
    fn unknown_lengths_show_dashes() {
        assert_eq!(format_or_unknown(None), UNKNOWN);
    }
}
