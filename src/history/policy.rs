//! What the player remembers and offers (S1.7.3, S1.7.4): when media counts as watched, which
//! positions are worth keeping, and where playback may resume.
//!
//! - Media shorter than [`MIN_RESUMABLE_DURATION`] and streams without a duration keep no
//!   position; their plays still count.
//! - Media is watched from `watched_percent` of its length or `watched_remaining_seconds` before
//!   its end, whichever comes first; it then starts from the beginning next time. Media shorter
//!   than [`MIN_RESUMABLE_DURATION`] uses the percentage only, or a 50-second clip would be
//!   watched as soon as it starts.
//! - Nothing is saved before [`RESUME_MARGIN`]: opening media and closing it at once (or while
//!   the resume toast shows) keeps where it was left.
//! - A saved position in the first or last [`RESUME_MARGIN`], or past the media's current end
//!   (it was trimmed or re-encoded), starts from the beginning.

use std::time::Duration;

use crate::db::media::Progress;
use crate::settings::{PlayerSettings, Settings};

/// Media shorter than this keeps no position.
pub const MIN_RESUMABLE_DURATION: Duration = Duration::from_secs(120);
/// A position this close to either end isn't worth resuming from.
pub const RESUME_MARGIN: Duration = Duration::from_secs(10);
/// How often a position is saved while playing, so a crash loses at most this much.
pub const SAVE_INTERVAL: Duration = Duration::from_secs(5);

/// When media counts as watched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WatchedRule {
    /// From this far in, in percent (clamped to 1–100).
    pub percent: u32,
    /// From this close to the end.
    pub remaining: Duration,
}

impl WatchedRule {
    /// The rule in the player settings.
    pub fn from_settings(player: &PlayerSettings) -> Self {
        Self {
            percent: player.watched_percent.clamp(1, 100),
            remaining: Duration::from_secs(player.watched_remaining_seconds.into()),
        }
    }

    /// Whether `position` into media `duration` long counts as watched.
    pub fn is_watched(self, position: Duration, duration: Duration) -> bool {
        let percent = f64::from(self.percent);
        let far_enough = position.as_secs_f64() * 100. >= duration.as_secs_f64() * percent;
        let near_end = duration >= MIN_RESUMABLE_DURATION
            && duration.saturating_sub(position) <= self.remaining;
        far_enough || near_end
    }
}

/// What the history may keep, from the settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rules {
    /// When media counts as watched.
    pub watched: WatchedRule,
    /// Whether positions are saved and resumed ("Remember the playback position").
    pub remember_position: bool,
}

impl Rules {
    /// The rules in `settings`.
    pub fn from_settings(settings: &Settings) -> Self {
        Self {
            watched: WatchedRule::from_settings(&settings.prefs.player),
            remember_position: settings.prefs.privacy.remember_playback_position,
        }
    }

    /// What to save for playback at `position` into media of `duration`; `None` when nothing
    /// is worth writing (no duration, or too early to replace where it was left).
    pub fn progress(self, position: Duration, duration: Option<Duration>) -> Option<Progress> {
        let duration = duration?;
        let watched = self.watched.is_watched(position, duration);
        if !watched && position < RESUME_MARGIN {
            return None;
        }
        let keep = self.remember_position && !watched && duration >= MIN_RESUMABLE_DURATION;
        Some(Progress { position: keep.then_some(position), duration: Some(duration), watched })
    }

    /// What to save when playback reached the end.
    pub fn finished(duration: Option<Duration>) -> Progress {
        Progress { position: None, duration, watched: true }
    }

    /// Where to resume media saved at `saved`, now `duration` long; `None` starts it over.
    pub fn resume_point(
        self,
        saved: Option<Duration>,
        duration: Option<Duration>,
    ) -> Option<Duration> {
        let (saved, duration) = (saved?, duration?);
        let resumable = self.remember_position
            && duration >= MIN_RESUMABLE_DURATION
            && saved >= RESUME_MARGIN
            && saved.saturating_add(RESUME_MARGIN) < duration;
        resumable.then_some(saved)
    }
}

/// A position as the player shows it: `12:34`, or `1:02:03` from an hour on.
pub fn clock(time: Duration) -> String {
    let seconds = time.as_secs();
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(secs: u64) -> Duration {
        Duration::from_secs(secs)
    }

    fn rules() -> Rules {
        Rules::from_settings(&Settings::default())
    }

    #[test]
    fn watched_at_95_percent_or_the_last_minute_whichever_comes_first() {
        let rule = rules().watched;
        // A 100-minute film: 95% (95:00) comes before the last minute (99:00).
        assert!(!rule.is_watched(secs(5_699), secs(6_000)));
        assert!(rule.is_watched(secs(5_700), secs(6_000)));
        // A 10-minute episode: the last minute (9:00) comes before 95% (9:30).
        assert!(rule.is_watched(secs(540), secs(600)));
        assert!(!rule.is_watched(secs(539), secs(600)));
        // A 30-minute show: 95% is 28:30, the last minute 29:00, so 95% comes first.
        assert!(rule.is_watched(secs(1_710), secs(1_800)));
        assert!(!rule.is_watched(secs(1_709), secs(1_800)));
    }

    #[test]
    fn positions_are_kept_only_for_long_unwatched_media() {
        let rules = rules();
        let hour = Some(secs(3_600));
        let kept = rules.progress(secs(600), hour);
        let expected = Progress { position: Some(secs(600)), duration: hour, watched: false };
        assert_eq!(kept, Some(expected));
        let short = rules.progress(secs(30), Some(secs(90))).expect("past the margin");
        assert_eq!((short.position, short.watched), (None, false));
        assert_eq!(rules.progress(secs(600), None), None);
        let done = rules.progress(secs(3_590), hour).expect("watched");
        assert_eq!((done.position, done.watched), (None, true));
    }

    #[test]
    fn the_first_seconds_replace_nothing() {
        let rules = rules();
        assert_eq!(rules.progress(secs(9), Some(secs(3_600))), None);
        assert!(rules.progress(secs(10), Some(secs(3_600))).is_some());
    }

    #[test]
    fn short_clips_are_watched_by_percentage_only() {
        let rule = rules().watched;
        assert!(!rule.is_watched(secs(0), secs(50)));
        assert!(rule.is_watched(secs(48), secs(50)));
    }

    #[test]
    fn forgetting_positions_keeps_watched() {
        let rules = Rules { remember_position: false, ..rules() };
        let hour = Some(secs(3_600));
        assert_eq!(rules.progress(secs(600), hour).and_then(|p| p.position), None);
        assert!(rules.progress(secs(3_590), hour).is_some_and(|p| p.watched));
        assert_eq!(rules.resume_point(Some(secs(600)), Some(secs(3_600))), None);
    }

    #[test]
    fn resume_skips_the_ends_and_trimmed_files() {
        let rules = rules();
        let hour = Some(secs(3_600));
        assert_eq!(rules.resume_point(Some(secs(754)), hour), Some(secs(754)));
        assert_eq!(rules.resume_point(Some(secs(5)), hour), None);
        assert_eq!(rules.resume_point(Some(secs(3_595)), hour), None);
        assert_eq!(rules.resume_point(Some(secs(754)), Some(secs(700))), None);
        assert_eq!(rules.resume_point(Some(secs(60)), Some(secs(100))), None);
        assert_eq!(rules.resume_point(None, hour), None);
        assert_eq!(rules.resume_point(Some(secs(754)), None), None);
    }

    #[test]
    fn odd_settings_are_clamped() {
        let mut settings = Settings::default();
        settings.prefs.player.watched_percent = 0;
        settings.prefs.player.watched_remaining_seconds = 0;
        let rule = WatchedRule::from_settings(&settings.prefs.player);
        assert!(!rule.is_watched(secs(0), secs(600)));
        assert!(rule.is_watched(secs(6), secs(600)));
    }

    #[test]
    fn clocks() {
        assert_eq!(clock(secs(754)), "12:34");
        assert_eq!(clock(secs(5)), "0:05");
        assert_eq!(clock(secs(3_723)), "1:02:03");
    }
}
