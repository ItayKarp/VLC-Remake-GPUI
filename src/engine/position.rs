//! The position the UI shows: clamped to the duration, monotonic between seeks, and throttled to
//! about 15 updates a second (S1.2.3).

use std::time::{Duration, Instant};

/// The shortest gap between two position updates sent to the UI (about 15 Hz).
pub const POSITION_INTERVAL: Duration = Duration::from_millis(66);

/// Turns the positions a backend reports into the ones the UI shows.
#[derive(Debug, Default)]
pub struct PositionTracker {
    duration: Option<Duration>,
    shown: Duration,
    last_sent: Option<Instant>,
}

impl PositionTracker {
    /// The position the UI shows now.
    pub fn shown(&self) -> Duration {
        self.shown
    }

    /// Sets the media's length, or `None` while unknown. Zero counts as unknown.
    pub fn set_duration(&mut self, duration: Option<Duration>) {
        self.duration = duration.filter(|duration| !duration.is_zero());
        self.shown = self.clamp(self.shown);
    }

    /// The media's length, if known.
    pub fn duration(&self) -> Option<Duration> {
        self.duration
    }

    /// The user seeked to `target`: show it at once, and from now on never less.
    pub fn seeked(&mut self, target: Duration, now: Instant) -> Duration {
        self.shown = self.clamp(target);
        self.last_sent = Some(now);
        self.shown
    }

    /// The backend reported `reported`. Returns the position to send to the UI, if one is due.
    /// Positions past the duration are clamped, and a position behind the shown one (the
    /// backend settling just before a seek target) doesn't move it back.
    pub fn report(&mut self, reported: Duration, now: Instant) -> Option<Duration> {
        let next = self.clamp(reported).max(self.shown);
        let due = self.last_sent.is_none_or(|last| now.duration_since(last) >= POSITION_INTERVAL);
        self.shown = next;
        if !due {
            return None;
        }
        self.last_sent = Some(now);
        Some(next)
    }

    /// Starts over at zero, for a new open.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    fn clamp(&self, time: Duration) -> Duration {
        self.duration.map_or(time, |duration| time.min(duration))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(secs: u64) -> Duration {
        Duration::from_secs(secs)
    }

    #[test]
    fn updates_are_throttled_but_the_latest_position_is_kept() {
        let start = Instant::now();
        let mut tracker = PositionTracker::default();
        assert_eq!(tracker.report(secs(1), start), Some(secs(1)));
        assert_eq!(tracker.report(secs(2), start + Duration::from_millis(10)), None);
        assert_eq!(tracker.shown(), secs(2));
        assert_eq!(tracker.report(secs(3), start + POSITION_INTERVAL), Some(secs(3)));
    }

    #[test]
    fn positions_past_the_end_are_clamped() {
        let mut tracker = PositionTracker::default();
        tracker.set_duration(Some(secs(10)));
        assert_eq!(tracker.report(secs(12), Instant::now()), Some(secs(10)));
    }

    #[test]
    fn a_position_slightly_behind_a_seek_target_does_not_move_back() {
        let start = Instant::now();
        let mut tracker = PositionTracker::default();
        tracker.report(secs(50), start);
        assert_eq!(tracker.seeked(secs(20), start), secs(20));
        let later = start + POSITION_INTERVAL;
        assert_eq!(tracker.report(Duration::from_millis(19_960), later), Some(secs(20)));
    }

    #[test]
    fn a_zero_duration_is_unknown() {
        let mut tracker = PositionTracker::default();
        tracker.set_duration(Some(Duration::ZERO));
        assert_eq!(tracker.duration(), None);
    }
}
