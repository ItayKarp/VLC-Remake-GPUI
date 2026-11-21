//! Player behavior (Settings → Playback; most of it comes with Stage 2 and S5.7): pausing for
//! other apps' audio (S1.5.6), resuming and marking media watched (S1.7.4), and the speed each
//! launch starts at (S2.7.4), queueing a file's folder with it (S3.1.3), and stopping after
//! each item and what to do when the queue finishes (S3.3.3), and bringing the last session
//! back at launch (S3.4.2). Repeat and Shuffle aren't here:
//! they belong to the queue, and Settings → Playback edits the queue's own state (S3.3.4).

use serde::{Deserialize, Serialize};

use super::super::lenient::lenient;

/// Media counts as watched from this far in, in percent, by default.
pub const DEFAULT_WATCHED_PERCENT: u32 = 95;
/// Media counts as watched this close to its end, in seconds, by default.
pub const DEFAULT_WATCHED_REMAINING_SECONDS: u32 = 60;
/// The playback speed a launch starts at, by default: normal speed.
pub const DEFAULT_SPEED: f32 = 1.0;

/// Player behavior.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent switches, each its own row in Settings → Playback"
)]
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
    /// Whether opening one file also queues the other media in its folder (S3.1.3).
    #[serde(deserialize_with = "lenient")]
    pub queue_siblings: QueueSiblings,
    /// "Stop after each item": the queue waits at the end of every item, and Play goes on to
    /// the next one.
    #[serde(deserialize_with = "lenient")]
    pub stop_after_each_item: bool,
    /// "When playback finishes": what happens when the queue's last item ends.
    #[serde(deserialize_with = "lenient")]
    pub when_finished: WhenFinished,
    /// "Continue where you left off" (S3.4.2): each launch brings back the last session's
    /// queue, paused at the saved position. Its row comes with S5.7.
    #[serde(deserialize_with = "lenient")]
    pub restore_session: bool,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self {
            pause_when_another_app_uses_audio: true,
            resume: ResumePolicy::default(),
            watched_percent: DEFAULT_WATCHED_PERCENT,
            watched_remaining_seconds: DEFAULT_WATCHED_REMAINING_SECONDS,
            default_speed: DEFAULT_SPEED,
            queue_siblings: QueueSiblings::default(),
            stop_after_each_item: false,
            when_finished: WhenFinished::default(),
            restore_session: true,
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

/// Whether opening one file also queues the other media in its folder, in natural order, with
/// the opened file current.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueSiblings {
    /// Only the opened file is queued.
    Never,
    /// When the file is a TV episode (`S01E02`, `1x02`), the folder's other episodes are queued.
    #[default]
    Episodes,
    /// The folder's other media is always queued.
    Always,
}

/// What happens when the queue's last item ends (S3.3.3). "Close the player" joins with the
/// library (Stage 4), the place it would close to; sleeping or shutting the computer down isn't
/// offered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WhenFinished {
    /// The player stays on the finished item.
    #[default]
    Stop,
    /// VLC quits.
    Quit,
}

impl WhenFinished {
    /// Every choice, in menu order.
    pub const ALL: [Self; 2] = [Self::Stop, Self::Quit];

    /// The choice's name in menus.
    pub fn label(self) -> &'static str {
        match self {
            Self::Stop => "Stop",
            Self::Quit => "Quit VLC",
        }
    }
}
