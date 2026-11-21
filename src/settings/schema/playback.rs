//! Player behavior (Settings → Playback; most of it comes with Stage 2 and S5.7): pausing for
//! other apps' audio (S1.5.6), resuming and marking media watched (S1.7.4), and the speed each
//! launch starts at (S2.7.4).

use serde::{Deserialize, Serialize};

use super::super::lenient::lenient;

/// Media counts as watched from this far in, in percent, by default.
pub const DEFAULT_WATCHED_PERCENT: u32 = 95;
/// Media counts as watched this close to its end, in seconds, by default.
pub const DEFAULT_WATCHED_REMAINING_SECONDS: u32 = 60;
/// The playback speed a launch starts at, by default: normal speed.
pub const DEFAULT_SPEED: f32 = 1.0;

/// Player behavior.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlayerSettings {
    /// "Pause when another app uses audio" (S1.5.6): pause while the OS asks for this app's
    /// audio to stop, and resume after.
    #[serde(deserialize_with = "lenient")]
    pub pause_when_another_app_uses_audio: bool,
    /// "Continue playback": what to do when media that was left partway opens again.
    #[serde(deserialize_with = "lenient")]
    pub resume: ResumePolicy,
    /// Media is watched once this far in, in percent (or [`Self::watched_remaining_seconds`]
    /// from the end, whichever comes first).
    #[serde(deserialize_with = "lenient")]
    pub watched_percent: u32,
    /// Media is watched once this close to its end, in seconds.
    #[serde(deserialize_with = "lenient")]
    pub watched_remaining_seconds: u32,
    /// "Default playback speed": the speed the player starts at. A speed the user picks lasts
    /// for the rest of the session and is never saved. The engine clamps it into range.
    #[serde(deserialize_with = "lenient")]
    pub default_speed: f32,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self {
            pause_when_another_app_uses_audio: true,
            resume: ResumePolicy::default(),
            watched_percent: DEFAULT_WATCHED_PERCENT,
            watched_remaining_seconds: DEFAULT_WATCHED_REMAINING_SECONDS,
            default_speed: DEFAULT_SPEED,
        }
    }
}

/// What happens when media that was left partway opens again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResumePolicy {
    /// Continue from where it was left.
    Always,
    /// Start from the beginning, with a "Resume from 12:34?" toast that continues.
    #[default]
    Ask,
    /// Always start from the beginning.
    Never,
}
