//! The position the UI shows: clamped to the duration, monotonic between seeks, and throttled to
//! about 15 updates a second (S1.2.3).

use std::time::{Duration, Instant};

/// The shortest gap between two position updates sent to the UI (about 15 Hz).
pub const POSITION_INTERVAL: Duration = Duration::from_millis(66);
/// How far [`PositionTracker::at`] runs on past the last report: further than the backend's
/// reports are apart, not so far that a stall shows subtitles ahead of the picture for long.
const MAX_RUN_ON: Duration = Duration::from_secs(1);

/// Turns the positions a backend reports into the ones the UI shows.
#[derive(Debug, Default)]
pub struct PositionTracker {
    duration: Option<Duration>,
    shown: Duration,
    last_sent: Option<Instant>,
    /// When [`Self::shown`] last moved, for running on between reports.
    moved: Option<Instant>,
}

impl PositionTracker {
    /// The position the UI shows now.
    pub fn shown(&self) -> Duration {
        self.shown
    }

    /// The position at `now`, between the backend's reports: the last one moved on at `rate`
    /// while `running`, for at most [`MAX_RUN_ON`]. Subtitles the app draws follow it (S6.2).
    pub fn at(&self, now: Instant, rate: f32, running: bool) -> Duration {
        let Some(moved) = self.moved.filter(|_| running) else { return self.shown };
        let elapsed = now.saturating_duration_since(moved).min(MAX_RUN_ON);
        self.clamp(self.shown + elapsed.mul_f32(rate.max(0.)))
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
        self.moved = Some(now);
        self.shown
    }

    /// The backend reported `reported`. Returns the position to send to the UI, if one is due.
    /// Positions past the duration are clamped, and a position behind the shown one (the
    /// backend settling just before a seek target) doesn't move it back.
    pub fn report(&mut self, reported: Duration, now: Instant) -> Option<Duration> {
        let next = self.clamp(reported).max(self.shown);
        let due = self.last_sent.is_none_or(|last| now.duration_since(last) >= POSITION_INTERVAL);
        self.shown = next;
        self.moved = Some(now);
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
    fn the_position_runs_on_between_reports_while_playing() {
        let start = Instant::now();
        let mut tracker = PositionTracker::default();
        tracker.report(secs(10), start);
        let later = start + Duration::from_millis(200);
        assert_eq!(tracker.at(later, 1.0, true), Duration::from_millis(10_200));
        assert_eq!(tracker.at(later, 2.0, true), Duration::from_millis(10_400));
        assert_eq!(tracker.at(later, 1.0, false), secs(10));
        assert_eq!(tracker.at(start + secs(5), 1.0, true), secs(11), "a stall runs on 1 s");
    }

    #[test]
    fn a_zero_duration_is_unknown() {
        let mut tracker = PositionTracker::default();
        tracker.set_duration(Some(Duration::ZERO));
        assert_eq!(tracker.duration(), None);
    }
}
