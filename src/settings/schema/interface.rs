//! Interface preferences (Settings → Interface, S5.4): the windows (where video plays, their size,
//! level and frame), what the player's title bar shows (S2.1.2), what the control bar's duration
//! readout shows (S2.4.3), whether the seek bar shows the subtitle timeline (S6.6), when the
//! controls hide (S2.8), and the now-playing notification.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::super::lenient::lenient;

/// The shortest "Hide after" delay. Zero would mean the controls never show on hover (S5.4
/// edge case), so it is not allowed.
const MIN_HIDE_AFTER_MS: u32 = 500;
/// The longest "Hide after" delay (Settings → Interface's slider ends at 10 s).
const MAX_HIDE_AFTER_MS: u32 = 10_000;
/// The default "Hide after" delay.
const DEFAULT_HIDE_AFTER_MS: u32 = 3_000;

/// Interface preferences.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent switches, each its own row in Settings → Interface"
)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InterfaceSettings {
    /// "Show the file name in the title bar": the title bar and the OS window title name the
    /// media while it's loaded; off, they always say "VLC Media Player".
    #[serde(deserialize_with = "lenient")]
    pub show_file_name_in_title_bar: bool,
    /// Whether the duration readout shows the length or the time left; clicking it switches.
    #[serde(deserialize_with = "lenient")]
    pub duration_readout: DurationReadout,
    /// "Show Subtitle Timeline on Seekbar" (S6.6): the seek bar shows a zoomed stretch around
    /// the playhead with the lines of the subtitle showing, when their times are known.
    #[serde(deserialize_with = "lenient")]
    pub subtitle_timeline: bool,
    /// "Auto-hide the controls": where the title and control bars hide while video plays.
    #[serde(deserialize_with = "lenient")]
    pub auto_hide_controls: AutoHide,
    /// "Hide after", in milliseconds: how long the pointer rests before the controls hide.
    /// Read it through [`InterfaceSettings::hide_after`], which keeps it in range.
    #[serde(deserialize_with = "lenient")]
    pub hide_controls_after_ms: u32,
    /// "Show controls in fullscreen": off, the controls only show in fullscreen when asked for
    /// with the "Show interface controls in fullscreen" key (I).
    #[serde(deserialize_with = "lenient")]
    pub show_controls_in_fullscreen: bool,
    /// Keeps the controls up while playback is paused, rather than hiding them as during play.
    #[serde(deserialize_with = "lenient")]
    pub keep_controls_while_paused: bool,
    /// "Play video inside main window": off, video plays in a window of its own (S5.4.1).
    #[serde(deserialize_with = "lenient")]
    pub play_video_in_main_window: bool,
    /// "Resize window to fit video": when the window showing video takes the video's size.
    #[serde(deserialize_with = "lenient")]
    pub resize_to_fit_video: FitVideo,
    /// "Remember window size and position": off, windows open centered at their default size.
    #[serde(deserialize_with = "lenient")]
    pub remember_window_geometry: bool,
    /// "Keep window above other windows", except while fullscreen.
    #[serde(deserialize_with = "lenient")]
    pub keep_above_other_windows: bool,
    /// "Show window decorations": off, the player has no title bar or window buttons.
    #[serde(deserialize_with = "lenient")]
    pub show_window_decorations: bool,
    /// When the OS is told that the next item started playing (S5.4.3).
    #[serde(deserialize_with = "lenient")]
    pub now_playing_notification: NowPlayingNotification,
    /// "Use a custom skin" (S5.4.4): read by nothing until skins exist (S8.14); its rows only
    /// show in builds with the `skins` feature.
    #[serde(deserialize_with = "lenient")]
    pub use_custom_skin: bool,
}

impl InterfaceSettings {
    /// How long the pointer rests before the controls hide, between 0.5 and 10 s.
    pub fn hide_after(&self) -> Duration {
        let ms = self.hide_controls_after_ms.clamp(MIN_HIDE_AFTER_MS, MAX_HIDE_AFTER_MS);
        Duration::from_millis(u64::from(ms))
    }
}

/// Where the controls hide by themselves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoHide {
    /// Only in fullscreen; a window keeps its bars.
    #[default]
    InFullscreenOnly,
    /// In a window too: the video fills the window and the bars float over it while shown.
    Always,
    /// Never: the controls stay.
    Never,
}

/// When the window showing video takes the video's size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FitVideo {
    /// The window keeps the size the user gave it.
    #[default]
    Never,
    /// For the first video played since launch.
    FirstVideo,
    /// Whenever a video starts or its size changes.
    EveryVideo,
}

/// When an OS notification says what started playing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NowPlayingNotification {
    /// Never.
    Never,
    /// While no VLC window is the active one.
    #[default]
    InBackground,
    /// For every item.
    Always,
}

/// What the control bar's duration readout shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DurationReadout {
    /// The media's length (`00:25:40`).
    #[default]
    Total,
    /// The media time left (`-00:23:30`), whatever the playback rate.
    Remaining,
}

impl DurationReadout {
    /// The other one.
    pub fn toggled(self) -> Self {
        match self {
            Self::Total => Self::Remaining,
            Self::Remaining => Self::Total,
        }
    }
}

impl Default for InterfaceSettings {
    fn default() -> Self {
        Self {
            show_file_name_in_title_bar: true,
            duration_readout: DurationReadout::default(),
            subtitle_timeline: false,
            auto_hide_controls: AutoHide::default(),
            hide_controls_after_ms: DEFAULT_HIDE_AFTER_MS,
            show_controls_in_fullscreen: true,
            keep_controls_while_paused: true,
            play_video_in_main_window: true,
            resize_to_fit_video: FitVideo::default(),
            remember_window_geometry: true,
            keep_above_other_windows: false,
            show_window_decorations: true,
            now_playing_notification: NowPlayingNotification::default(),
            use_custom_skin: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hide_delay_stays_in_range() {
        let with = |ms| InterfaceSettings { hide_controls_after_ms: ms, ..Default::default() };
        assert_eq!(InterfaceSettings::default().hide_after(), Duration::from_secs(3));
        assert_eq!(with(0).hide_after(), Duration::from_millis(500));
        assert_eq!(with(60_000).hide_after(), Duration::from_secs(10));
    }
}
