//! How long queued media is (S3.2.2): each row's duration and the header's total.
//!
//! Lengths are learned lazily, from a background probe of the file's headers or from the player
//! once it plays an item, and kept per media, so a file queued twice is probed once. Until every
//! length is known the total shows the known sum with a "+" ("4 items - 1h 12m+"), and media
//! that has no length (a live stream) keeps it that way.

use std::collections::HashMap;
use std::time::Duration;

use super::Entry;
use crate::app::media::Media;

const SECONDS_PER_MINUTE: u64 = 60;
const SECONDS_PER_HOUR: u64 = 3600;

/// What is known about one media's length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Length {
    /// It hasn't been asked for.
    Unasked,
    /// It's being probed.
    Probing,
    /// It is this long.
    Known(Duration),
    /// It has none, or it couldn't be read.
    Unknown,
}

/// The lengths learned so far.
#[derive(Debug, Default)]
pub struct Lengths {
    lengths: HashMap<Media, Length>,
}

impl Lengths {
    /// What is known about `media`'s length.
    pub fn get(&self, media: &Media) -> Length {
        self.lengths.get(media).copied().unwrap_or(Length::Unasked)
    }

    /// `media`'s length, when known.
    pub fn duration(&self, media: &Media) -> Option<Duration> {
        match self.get(media) {
            Length::Known(duration) => Some(duration),
            _ => None,
        }
    }

    /// Records `media`'s length (`None`: it has none). Returns whether that changed anything.
    pub fn set(&mut self, media: Media, duration: Option<Duration>) -> bool {
        let length = duration.filter(|d| !d.is_zero()).map_or(Length::Unknown, Length::Known);
        self.lengths.insert(media, length) != Some(length)
    }

    /// The media in `entries` nobody asked about yet, each once, now marked as being probed.
    pub fn take_unasked(&mut self, entries: &[Entry]) -> Vec<Media> {
        let mut unasked = Vec::new();
        for entry in entries {
            if self.get(&entry.media) == Length::Unasked {
                self.lengths.insert(entry.media.clone(), Length::Probing);
                unasked.push(entry.media.clone());
            }
        }
        unasked
    }

    /// Forgets the lengths of media that is no longer queued, and the probes that were
    /// waiting for it (the prober drops its backlog too).
    pub fn retain_queued(&mut self, entries: &[Entry]) {
        let queued: std::collections::HashSet<&Media> = entries.iter().map(|e| &e.media).collect();
        self.lengths
            .retain(|media, length| queued.contains(media) && !matches!(length, Length::Probing));
    }

    /// The header's summary of `entries`.
    pub fn total(&self, entries: &[Entry]) -> Total {
        let mut total = Total { count: entries.len(), known: Duration::ZERO, complete: true };
        for entry in entries {
            match self.get(&entry.media) {
                Length::Known(duration) => total.known += duration,
                _ => total.complete = false,
            }
        }
        total
    }
}

/// How many entries the queue has and how long they last together.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Total {
    /// The number of entries.
    pub count: usize,
    /// The sum of the lengths that are known.
    pub known: Duration,
    /// Whether every length is known.
    pub complete: bool,
}

impl Total {
    /// "4 items - 2h 3m", with a "+" while some lengths are missing; just the count while none
    /// is known.
    pub fn label(self) -> String {
        let items = match self.count {
            1 => "1 item".to_owned(),
            count => format!("{count} items"),
        };
        if self.known.is_zero() {
            return items;
        }
        let plus = if self.complete { "" } else { "+" };
        format!("{items} - {}{plus}", hours_and_minutes(self.known))
    }
}

/// `duration` as "2h 3m", "45m", or "40s" under a minute.
fn hours_and_minutes(duration: Duration) -> String {
    let seconds = duration.as_secs();
    let (hours, minutes) = (seconds / SECONDS_PER_HOUR, seconds / SECONDS_PER_MINUTE % 60);
    match (hours, minutes) {
        (0, 0) => format!("{seconds}s"),
        (0, minutes) => format!("{minutes}m"),
        (hours, minutes) => format!("{hours}h {minutes}m"),
    }
}

/// A row's duration: "23:06", or "1:02:03" past an hour.
pub fn row_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    let (hours, minutes, seconds) =
        (seconds / SECONDS_PER_HOUR, seconds / SECONDS_PER_MINUTE % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::queue::Queue;

    fn file(name: &str) -> Media {
        Media::File(PathBuf::from(name))
    }

    fn mins(minutes: u64, seconds: u64) -> Duration {
        Duration::from_secs(minutes * 60 + seconds)
    }

    #[test]
    fn the_total_is_computed_and_marks_missing_lengths() {
        let mut q = Queue::default();
        q.replace(vec![file("a"), file("b"), file("c"), file("d")]);
        let mut lengths = Lengths::default();
        assert_eq!(lengths.total(q.entries()).label(), "4 items");
        lengths.set(file("a"), Some(mins(23, 6)));
        lengths.set(file("b"), Some(mins(24, 18)));
        lengths.set(file("c"), Some(mins(25, 0)));
        assert_eq!(lengths.total(q.entries()).label(), "4 items - 1h 12m+");
        lengths.set(file("d"), Some(mins(22, 35)));
        // The design's rows add up to 1h 35m (its "2h 3m" is wrong).
        assert_eq!(lengths.total(q.entries()).label(), "4 items - 1h 34m");
    }

    #[test]
    fn short_totals_and_single_items() {
        let total = Total { count: 1, known: Duration::from_secs(40), complete: true };
        assert_eq!(total.label(), "1 item - 40s");
        let total = Total { count: 0, known: Duration::ZERO, complete: true };
        assert_eq!(total.label(), "0 items");
    }

    #[test]
    fn each_media_is_probed_once() {
        let mut q = Queue::default();
        q.replace(vec![file("a"), file("a"), file("b")]);
        let mut lengths = Lengths::default();
        assert_eq!(lengths.take_unasked(q.entries()), [file("a"), file("b")]);
        assert_eq!(lengths.take_unasked(q.entries()), Vec::<Media>::new());
        assert!(lengths.set(file("a"), None));
        assert_eq!(lengths.get(&file("a")), Length::Unknown);
        assert!(!lengths.set(file("a"), Some(Duration::ZERO)), "zero means none");
    }

    #[test]
    fn dequeued_media_and_waiting_probes_are_forgotten() {
        let mut q = Queue::default();
        q.replace(vec![file("a"), file("b")]);
        let mut lengths = Lengths::default();
        lengths.take_unasked(q.entries());
        lengths.set(file("a"), Some(mins(1, 0)));
        q.replace(vec![file("a"), file("c")]);
        lengths.retain_queued(q.entries());
        assert_eq!(lengths.get(&file("a")), Length::Known(mins(1, 0)));
        assert_eq!(lengths.get(&file("b")), Length::Unasked);
    }

    #[test]
    fn row_durations_as_designed() {
        assert_eq!(row_duration(mins(23, 6)), "23:06");
        assert_eq!(row_duration(Duration::from_secs(5)), "0:05");
        assert_eq!(row_duration(mins(62, 3)), "1:02:03");
    }
}
