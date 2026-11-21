//! How far each media was watched (S4.1.2), as the library cards show it: unwatched, in progress
//! with an orange sliver, or watched and dimmed.
//!
//! It is derived from what the history saved (S1.7), never stored, and "watched" is the one
//! [`WatchedRule`] of S1.7.4: a position the rule now counts as watched (the setting was lowered
//! since it was saved) shows as watched.

use crate::db::media::Saved;
use crate::history::policy::WatchedRule;

/// How far a media was watched.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WatchState {
    /// Never played past the resume margin, or marked unwatched.
    Unwatched,
    /// Left partway, at this fraction of its length (above 0, below 1).
    InProgress(f32),
    /// Played to the end, or marked watched.
    Watched,
}

impl WatchState {
    /// The state of a media the history saved as `saved`, with `rule` deciding what is watched.
    pub fn of(saved: &Saved, rule: WatchedRule) -> Self {
        if saved.watched {
            return Self::Watched;
        }
        let (Some(position), Some(duration)) = (saved.position, saved.duration) else {
            return Self::Unwatched;
        };
        if duration.is_zero() {
            return Self::Unwatched;
        }
        if rule.is_watched(position, duration) {
            return Self::Watched;
        }
        let fraction = position.as_secs_f32() / duration.as_secs_f32();
        if fraction > 0. { Self::InProgress(fraction.min(1.)) } else { Self::Unwatched }
    }

    /// Whether Mark as Watched would change it, so the file menu offers Mark as Unwatched instead
    /// (S4.8.1) when it wouldn't.
    pub fn is_watched(self) -> bool {
        self == Self::Watched
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::settings::Settings;

    fn rule() -> WatchedRule {
        WatchedRule::from_settings(&Settings::default().prefs.player)
    }

    fn saved(position: Option<u64>, duration: Option<u64>) -> Saved {
        Saved {
            position: position.map(Duration::from_secs),
            duration: duration.map(Duration::from_secs),
            ..Saved::default()
        }
    }

    #[test]
    fn a_saved_position_is_progress_through_the_media() {
        let state = WatchState::of(&saved(Some(900), Some(3_600)), rule());
        assert_eq!(state, WatchState::InProgress(0.25));
    }

    #[test]
    fn nothing_saved_is_unwatched() {
        assert_eq!(WatchState::of(&Saved::default(), rule()), WatchState::Unwatched);
        assert_eq!(WatchState::of(&saved(None, Some(3_600)), rule()), WatchState::Unwatched);
        assert_eq!(WatchState::of(&saved(Some(0), Some(3_600)), rule()), WatchState::Unwatched);
        assert_eq!(WatchState::of(&saved(Some(5), Some(0)), rule()), WatchState::Unwatched);
    }

    #[test]
    fn the_watched_flag_wins() {
        let marked = Saved { watched: true, ..saved(Some(900), Some(3_600)) };
        assert!(WatchState::of(&marked, rule()).is_watched());
    }

    #[test]
    fn the_watched_threshold_is_the_history_s() {
        // 95% of an hour is 57:00: saved before the threshold, a lower setting now passes it.
        let saved = saved(Some(3_000), Some(3_600));
        assert!(!WatchState::of(&saved, rule()).is_watched());
        let lowered = WatchedRule { percent: 80, ..rule() };
        assert!(WatchState::of(&saved, lowered).is_watched());
    }
}
