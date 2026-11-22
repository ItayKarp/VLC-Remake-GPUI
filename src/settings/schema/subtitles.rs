//! Subtitle settings (Settings → Subtitles, "Subtitle Defaults"; S1.6.3, rows in S5.8.3): whether
//! subtitles show and in which language, the text encoding, which files load with a video, and
//! how subtitles look.

use serde::{Deserialize, Serialize};

use super::super::lenient::lenient;
use crate::engine::subtitle_style::{FileMatching, Outline, RelativeSize};

/// "Font size" in pixels by default, used once "Size values" is Pixels.
pub const DEFAULT_PIXEL_SIZE: u32 = 32;
/// "Text opacity" by default, in percent.
pub const DEFAULT_TEXT_OPACITY_PERCENT: u32 = 100;

/// Subtitle preferences that follow the user between machines.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent switches, each its own row in Settings → Subtitles"
)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SubtitleSettings {
    /// "Enable subtitles": new media shows subtitles in a preferred language, or the media's
    /// default ones. Off, only forced subtitles in the audio's language show.
    #[serde(deserialize_with = "lenient")]
    pub enabled: bool,
    /// "Preferred language": language codes (`en`, `heb`), most preferred first.
    #[serde(deserialize_with = "lenient")]
    pub preferred_languages: Vec<String>,
    /// "Text encoding": the character set text subtitles are read in (`Windows-1255`); empty
    /// guesses.
    #[serde(deserialize_with = "lenient")]
    pub text_encoding: String,
    /// "Load subtitle files automatically": subtitle files next to a video load with it.
    #[serde(deserialize_with = "lenient")]
    pub load_files: bool,
    /// "File name matching": which of those files.
    #[serde(deserialize_with = "lenient")]
    pub file_matching: FileMatching,
    /// "Font": the family; empty for the system's.
    #[serde(deserialize_with = "lenient")]
    pub font: String,
    /// "Size values": relative to the video, or in pixels.
    #[serde(deserialize_with = "lenient")]
    pub size_values: SizeValues,
    /// "Font size" while [`Self::size_values`] is relative.
    #[serde(deserialize_with = "lenient")]
    pub relative_size: RelativeSize,
    /// "Font size" in pixels while [`Self::size_values`] is pixels.
    #[serde(deserialize_with = "lenient")]
    pub pixel_size: u32,
    /// "Bold".
    #[serde(deserialize_with = "lenient")]
    pub bold: bool,
    /// "Text color", `#RRGGBB`.
    #[serde(deserialize_with = "lenient")]
    pub text_color: String,
    /// "Text opacity", in percent.
    #[serde(deserialize_with = "lenient")]
    pub text_opacity_percent: u32,
    /// "Outline thickness".
    #[serde(deserialize_with = "lenient")]
    pub outline: Outline,
    /// "Outline color", `#RRGGBB`.
    #[serde(deserialize_with = "lenient")]
    pub outline_color: String,
    /// "Shadow".
    #[serde(deserialize_with = "lenient")]
    pub shadow: bool,
}

impl Default for SubtitleSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            preferred_languages: Vec::new(),
            text_encoding: String::new(),
            load_files: true,
            file_matching: FileMatching::default(),
            font: String::new(),
            size_values: SizeValues::default(),
            relative_size: RelativeSize::default(),
            pixel_size: DEFAULT_PIXEL_SIZE,
            bold: false,
            text_color: "#FFFFFF".into(),
            text_opacity_percent: DEFAULT_TEXT_OPACITY_PERCENT,
            outline: Outline::default(),
            outline_color: "#000000".into(),
            shadow: true,
        }
    }
}

/// "Size values": how "Font size" is given.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SizeValues {
    /// Relative to the video's height, so text keeps its place as the window resizes.
    #[default]
    Relative,
    /// A fixed height in pixels.
    Pixels,
}
