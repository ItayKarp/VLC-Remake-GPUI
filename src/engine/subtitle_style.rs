//! How subtitles look (Settings → Subtitles' Appearance, S5.8.3), and how subtitle files are
//! found and read (Loading Subtitles, Text encoding).
//!
//! libVLC 3's text renderer reads its style once, when the player's instance is made, so the
//! style applies from the next launch. Styled (ASS/SSA) subtitles keep their own look. Which
//! subtitle files load is decided as each media opens ([`super::sidecar`]), and their encoding
//! is an option of each media.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::subtitle_text::EncodingChoice;

/// White, the renderer's default text color.
pub const WHITE: u32 = 0x00FF_FFFF;
/// Black, the renderer's default outline color.
pub const BLACK: u32 = 0;
/// Fully opaque.
pub const OPAQUE: u8 = 255;

/// The text renderer's style.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubtitleStyle {
    /// The font family; empty for the renderer's own.
    pub font: String,
    /// How big the text is.
    pub size: TextSize,
    /// Bold text.
    pub bold: bool,
    /// The text color, `0xRRGGBB`.
    pub color: u32,
    /// The text's opacity, 0 (invisible) to 255.
    pub opacity: u8,
    /// The outline around each letter.
    pub outline: Outline,
    /// The outline's color, `0xRRGGBB`.
    pub outline_color: u32,
    /// A soft shadow under the text.
    pub shadow: bool,
}

impl Default for SubtitleStyle {
    fn default() -> Self {
        Self {
            font: String::new(),
            size: TextSize::Relative(RelativeSize::Automatic),
            bold: false,
            color: WHITE,
            opacity: OPAQUE,
            outline: Outline::Normal,
            outline_color: BLACK,
            shadow: true,
        }
    }
}

/// Subtitle text size: relative to the video's height, or in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextSize {
    /// Grows and shrinks with the video.
    Relative(RelativeSize),
    /// This many pixels high, whatever the video's size.
    Pixels(u32),
}

/// A size relative to the video ("Font size" with "Size values" Relative).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs, reason = "the sizes are their own names")]
pub enum RelativeSize {
    /// The renderer's own choice for the video.
    #[default]
    Automatic,
    Smaller,
    Small,
    Normal,
    Large,
    Larger,
}

impl RelativeSize {
    /// libVLC's `freetype-rel-fontsize`: the video's height over the text's; 0 is automatic.
    pub fn libvlc_value(self) -> u32 {
        match self {
            Self::Automatic => 0,
            Self::Smaller => 20,
            Self::Small => 18,
            Self::Normal => 16,
            Self::Large => 12,
            Self::Larger => 6,
        }
    }
}

/// "Outline thickness".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs, reason = "the thicknesses are their own names")]
pub enum Outline {
    None,
    Thin,
    #[default]
    Normal,
    Thick,
}

impl Outline {
    /// libVLC's `freetype-outline-thickness`.
    pub fn libvlc_value(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Thin => 2,
            Self::Normal => 4,
            Self::Thick => 6,
        }
    }
}

/// "File name matching": which subtitle files next to a video load with it
/// ([`super::sidecar`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileMatching {
    /// Only `Movie.srt` for `Movie.mkv`.
    Exact,
    /// The title with more after it: `Movie.en.srt`, `Movie.forced.srt`.
    #[default]
    TitleWithExtra,
    /// Any name containing the title: `[Group] Movie (2020).srt`.
    ContainingTitle,
    /// Every subtitle file in the folder.
    AnyInFolder,
}

/// How each media looks for and reads subtitle files.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SubtitleLoading {
    /// Which files next to the media load with it; `None` loads none.
    pub files: Option<FileMatching>,
    /// The character set text subtitles are read in (`Windows-1255`); empty guesses each file's
    /// ([`super::subtitle_text`]).
    pub encoding: String,
    /// The folder copies of subtitle files converted to UTF-8 go in; `None` converts none.
    pub converted: Option<PathBuf>,
}

impl SubtitleLoading {
    /// How a file's encoding is decided when the user hasn't chosen one for it.
    pub fn encoding_choice(&self) -> EncodingChoice {
        if self.encoding.is_empty() { EncodingChoice::Guess } else { EncodingChoice::Setting }
    }
}
