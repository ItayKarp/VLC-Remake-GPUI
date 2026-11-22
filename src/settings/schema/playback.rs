//! Player behavior (Settings → Playback, S5.7): pausing for other apps' audio (S1.5.6),
//! resuming and marking media watched (S1.7.4), the speed each launch starts at (S2.7.4),
//! queueing a file's folder with it (S3.1.3), stopping after each item and what to do when the
//! queue finishes (S3.3.3), bringing the last session back at launch (S3.4.2), and how new media
//! opens: playing or paused, its stream quality, disc menu language, default tracks and how
//! precisely it seeks. Repeat and Shuffle aren't here: they belong to the queue, and Settings →
//! Playback edits the queue's own state through [`super::LiveState`] (S3.3.4).

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
    /// "Continue where you left off": what to do when media that was left partway opens again.
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
    /// "Bring back the last queue" (S3.4.2): each launch brings back the last session's
    /// queue, paused at the saved position.
    #[serde(deserialize_with = "lenient")]
    pub restore_session: bool,
    /// "Start playing as soon as a file opens": off, media the user opens waits paused at its
    /// start.
    #[serde(deserialize_with = "lenient")]
    pub start_playing: bool,
    /// "Preferred video quality": the resolution adaptive streams (HLS, DASH) pick.
    #[serde(deserialize_with = "lenient")]
    pub video_quality: VideoQuality,
    /// "Preferred menu language": DVD and Blu-ray menus in this language code (`en`); empty
    /// lets the disc choose.
    #[serde(deserialize_with = "lenient")]
    pub menu_language: String,
    /// "Default audio track": the track to play, counted from 1, when no preferred language
    /// has one; 0 keeps the media's own choice.
    #[serde(deserialize_with = "lenient")]
    pub default_audio_track: u32,
    /// "Default subtitle track": as [`Self::default_audio_track`], for subtitles.
    #[serde(deserialize_with = "lenient")]
    pub default_subtitle_track: u32,
    /// "Seek precisely": seeks land on the exact frame; off, on the nearest keyframe, faster.
    #[serde(deserialize_with = "lenient")]
    pub seek_precisely: bool,
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
            start_playing: true,
            video_quality: VideoQuality::default(),
            menu_language: String::new(),
            default_audio_track: 0,
            default_subtitle_track: 0,
            seek_precisely: true,
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

/// The resolution adaptive streams pick among the ones they offer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoQuality {
    /// The highest the connection keeps up with.
    #[default]
    Best,
    /// Up to 1080 lines.
    Full,
    /// Up to 720 lines.
    High,
    /// Up to 576 lines.
    Standard,
    /// Up to 360 lines.
    Low,
    /// Up to 240 lines, to save data.
    Lowest,
}

impl VideoQuality {
    /// The most lines a picked stream may have; `None` for no limit.
    pub fn max_lines(self) -> Option<u32> {
        match self {
            Self::Best => None,
            Self::Full => Some(1080),
            Self::High => Some(720),
            Self::Standard => Some(576),
            Self::Low => Some(360),
            Self::Lowest => Some(240),
        }
    }
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
