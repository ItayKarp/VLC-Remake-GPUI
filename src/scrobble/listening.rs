//! When a play counts as a scrobble (S8.17.1), as last.fm asks: the track is longer than 30
//! seconds and was listened to for half its length or four minutes, whichever comes first. A
//! track of unknown length (a radio stream) counts after four minutes.
//!
//! Listening time is what was heard, not where playback got to: it grows with the position while
//! it moves forward in small steps, so seeking past the middle doesn't count, and pausing
//! doesn't either.

use std::time::Duration;

/// Tracks this short are never scrobbled.
pub const MIN_LENGTH: Duration = Duration::from_secs(30);
/// Listening this long always counts.
pub const ENOUGH: Duration = Duration::from_mins(4);
/// The largest step forward that is still playback rather than a seek: positions come about 15
/// times a second, faster media and a busy window space them out.
const MAX_STEP: Duration = Duration::from_secs(2);

/// How long one play of a track was listened to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Listening {
    /// Time heard so far.
    heard: Duration,
    /// The last position seen, to measure the next step from.
    last: Option<Duration>,
}

impl Listening {
    /// Takes in the position playback is at.
    pub fn at(&mut self, position: Duration) {
        if let Some(step) = self.last.and_then(|last| position.checked_sub(last))
            && step <= MAX_STEP
        {
            self.heard += step;
        }
        self.last = Some(position);
    }

    /// Playback paused or jumped: the next position starts a new stretch.
    pub fn interrupted(&mut self) {
        self.last = None;
    }

    /// Whether the play counts, for a track of `length` (`None` when unknown).
    pub fn counts(&self, length: Option<Duration>) -> bool {
        match length {
            Some(length) if length <= MIN_LENGTH => false,
            Some(length) => self.heard >= (length / 2).min(ENOUGH),
            None => self.heard >= ENOUGH,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(seconds: u64) -> Duration {
        Duration::from_secs(seconds)
    }

    /// Listens from `from` to `to`, a second at a time.
    fn play(listening: &mut Listening, from: u64, to: u64) {
        for second in from..=to {
            listening.at(secs(second));
        }
    }

    #[test]
    fn half_of_a_short_track_counts() {
        let mut listening = Listening::default();
        play(&mut listening, 0, 99);
        assert!(!listening.counts(Some(secs(200))));
        listening.at(secs(100));
        assert!(listening.counts(Some(secs(200))));
    }

    #[test]
    fn four_minutes_of_a_long_track_count() {
        let mut listening = Listening::default();
        play(&mut listening, 0, 240);
        assert!(listening.counts(Some(secs(3600))));
        assert!(listening.counts(None));
    }

    #[test]
    fn tracks_of_thirty_seconds_or_less_never_count() {
        let mut listening = Listening::default();
        play(&mut listening, 0, 30);
        assert!(!listening.counts(Some(secs(30))));
    }

    #[test]
    fn seeking_and_pausing_add_nothing() {
        let mut listening = Listening::default();
        play(&mut listening, 0, 10);
        listening.at(secs(150));
        listening.interrupted();
        play(&mut listening, 150, 160);
        listening.at(secs(20));
        // Twenty seconds were heard: half of 40 s, not of 42 s.
        assert!(listening.counts(Some(secs(40))));
        assert!(!listening.counts(Some(secs(42))));
    }
}
