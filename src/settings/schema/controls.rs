//! Controls preferences (Settings → Controls; its rows come with S5.6): how far the seek keys
//! jump (S2.3.4, S5.6.1), and what clicks and the scroll wheel do over the video (S2.5.5,
//! S2.10.1, S5.6.2).

use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::super::lenient::lenient;

/// One of the four skip amounts the seek keys use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SkipLength {
    /// Shift with an arrow: 3 s by default.
    VeryShort,
    /// An arrow alone: 10 s by default.
    Short,
    /// Alt (Option) with an arrow: 60 s by default.
    Medium,
    /// Ctrl (Cmd on macOS) with an arrow: 300 s by default.
    Long,
}

/// Controls preferences.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ControlsSettings {
    /// How far the seek keys jump.
    #[serde(deserialize_with = "lenient")]
    pub skip: SkipSeconds,
    /// What the scroll wheel does over the video.
    #[serde(deserialize_with = "lenient")]
    pub wheel: WheelAction,
    /// What scrolling sideways (a tilted wheel, a trackpad) does over the video.
    #[serde(deserialize_with = "lenient")]
    pub horizontal_wheel: HorizontalWheelAction,
    /// What a single click on the video does.
    #[serde(deserialize_with = "lenient")]
    pub click: ClickAction,
    /// What a double click on the video does.
    #[serde(deserialize_with = "lenient")]
    pub double_click: DoubleClickAction,
    /// "Reverse scroll direction": scrolling over the video turns the other way (down is
    /// louder, left is forward).
    #[serde(deserialize_with = "lenient")]
    pub reverse_scroll: bool,
}

/// What a single click on the video does ("Mouse → Single click").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClickAction {
    /// Plays or pauses.
    #[default]
    PlayPause,
    /// Nothing.
    Nothing,
}

/// What a double click on the video does ("Mouse → Double click").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoubleClickAction {
    /// Enters or leaves fullscreen.
    #[default]
    ToggleFullscreen,
    /// Nothing.
    Nothing,
}

/// What scrolling sideways does over the video ("Mouse → Horizontal scroll").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HorizontalWheelAction {
    /// Seeks by the very short skip per step, as in VLC.
    #[default]
    Seek,
    /// Nothing.
    Nothing,
}

/// What the scroll wheel does over the video ("Mouse → Scroll wheel").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WheelAction {
    /// Steps the volume by the "Volume step", as in VLC.
    #[default]
    Volume,
    /// Nothing.
    Nothing,
}

/// The four skip amounts, in seconds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SkipSeconds {
    /// "Very short skip".
    #[serde(deserialize_with = "lenient")]
    pub very_short: u32,
    /// "Short skip".
    #[serde(deserialize_with = "lenient")]
    pub short: u32,
    /// "Medium skip".
    #[serde(deserialize_with = "lenient")]
    pub medium: u32,
    /// "Long skip".
    #[serde(deserialize_with = "lenient")]
    pub long: u32,
}

impl Default for SkipSeconds {
    fn default() -> Self {
        Self { very_short: 3, short: 10, medium: 60, long: 300 }
    }
}

impl ControlsSettings {
    /// How far a `length` skip jumps; at least a second, so a key never does nothing.
    pub fn skip(&self, length: SkipLength) -> Duration {
        let seconds = match length {
            SkipLength::VeryShort => self.skip.very_short,
            SkipLength::Short => self.skip.short,
            SkipLength::Medium => self.skip.medium,
            SkipLength::Long => self.skip.long,
        };
        Duration::from_secs(u64::from(seconds.max(1)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_vlc() {
        let controls = ControlsSettings::default();
        let skips =
            [SkipLength::VeryShort, SkipLength::Short, SkipLength::Medium, SkipLength::Long]
                .map(|length| controls.skip(length).as_secs());
        assert_eq!(skips, [3, 10, 60, 300]);
    }

    #[test]
    fn a_zero_skip_still_moves() {
        let skip = SkipSeconds { short: 0, ..SkipSeconds::default() };
        let controls = ControlsSettings { skip, ..ControlsSettings::default() };
        assert_eq!(controls.skip(SkipLength::Short), Duration::from_secs(1));
    }
}
