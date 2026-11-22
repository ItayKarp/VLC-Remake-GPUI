//! How new media opens, beyond where it starts: the stream quality adaptive streams pick, the
//! language disc menus show in and how precisely seeks land (Settings → Playback, S5.7), and
//! whether video is decoded, its picture adjustments and its deinterlacing (Settings → Video,
//! S5.8.1), whether audio is (Settings → Audio, S5.8.2), and which subtitle files load and how
//! they are read (Settings → Subtitles, S5.8.3).

use serde::{Deserialize, Serialize};

use super::language::Language;
use super::subtitle_style::{FileMatching, SubtitleLoading};

/// Options every media opened from now on gets.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent options, each its own row in Settings → Playback, Video or Audio"
)]
#[derive(Clone, Debug, PartialEq)]
pub struct MediaOptions {
    /// The most lines an adaptive stream (HLS, DASH) may pick; `None` for the best available.
    pub max_lines: Option<u32>,
    /// The language DVD and Blu-ray menus show in; `None` lets the disc choose.
    pub menu_language: Option<Language>,
    /// Whether seeks land on the exact moment (`true`) or the nearest keyframe, faster.
    pub precise_seeks: bool,
    /// Whether video tracks are decoded at all; off plays the sound only, saving the CPU.
    pub video: bool,
    /// Whether audio tracks are decoded at all; off plays the picture silently.
    pub audio: bool,
    /// The picture's hue, brightness, contrast, saturation and gamma.
    pub picture: Picture,
    /// Whether interlaced video is deinterlaced, and how.
    pub deinterlace: Deinterlacing,
    /// Which subtitle files load with the media, and how they are read.
    pub subtitles: SubtitleLoading,
}

impl Default for MediaOptions {
    fn default() -> Self {
        Self {
            max_lines: None,
            menu_language: None,
            precise_seeks: true,
            video: true,
            audio: true,
            picture: Picture::default(),
            deinterlace: Deinterlacing::default(),
            subtitles: SubtitleLoading {
                files: Some(FileMatching::default()),
                ..SubtitleLoading::default()
            },
        }
    }
}

/// libVLC's `adjust` filter's settings, in its own units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Picture {
    /// Degrees, −180 to 180; 0 leaves colors as they are.
    pub hue: f32,
    /// 0 to 2; 1 is unchanged.
    pub brightness: f32,
    /// 0 to 2; 1 is unchanged.
    pub contrast: f32,
    /// 0 to 3; 1 is unchanged, 0 is greyscale.
    pub saturation: f32,
    /// 0.01 to 10; 1 is unchanged.
    pub gamma: f32,
}

impl Picture {
    /// The picture as the media has it.
    pub const NEUTRAL: Self =
        Self { hue: 0., brightness: 1., contrast: 1., saturation: 1., gamma: 1. };

    /// Whether every adjustment leaves the picture as it is, so no filter is needed.
    pub fn is_neutral(&self) -> bool {
        *self == Self::NEUTRAL
    }
}

impl Default for Picture {
    fn default() -> Self {
        Self::NEUTRAL
    }
}

/// Whether interlaced video is deinterlaced, and how.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Deinterlacing {
    /// When.
    pub mode: Deinterlace,
    /// How.
    pub method: DeinterlaceMethod,
}

/// When video is deinterlaced ("Deinterlacing" in Settings → Video).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Deinterlace {
    /// When the video says it is interlaced.
    #[default]
    Automatic,
    /// Always.
    On,
    /// Never.
    Off,
}

/// How video is deinterlaced ("Method"): libVLC's `deinterlace-mode` values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeinterlaceMethod {
    /// libVLC picks one for the video.
    #[default]
    Automatic,
    /// Drops every other line.
    Discard,
    /// Blends the two fields.
    Blend,
    /// Averages the two fields.
    Mean,
    /// Shows each field in turn, doubling the frame rate.
    Bob,
    /// Bob, interpolating the missing lines.
    Linear,
    /// An edge-following interpolation.
    X,
    /// Motion-adaptive, at the video's frame rate.
    Yadif,
    /// Motion-adaptive, at twice the frame rate.
    Yadif2x,
    /// Fields shown with persistence, like a CRT.
    Phosphor,
    /// Reverses the telecine of film shown at NTSC rates.
    Ivtc,
}

impl DeinterlaceMethod {
    /// libVLC's name for the method.
    pub fn libvlc_name(self) -> &'static str {
        match self {
            Self::Automatic => "auto",
            Self::Discard => "discard",
            Self::Blend => "blend",
            Self::Mean => "mean",
            Self::Bob => "bob",
            Self::Linear => "linear",
            Self::X => "x",
            Self::Yadif => "yadif",
            Self::Yadif2x => "yadif2x",
            Self::Phosphor => "phosphor",
            Self::Ivtc => "ivtc",
        }
    }
}
