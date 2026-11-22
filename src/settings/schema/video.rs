//! Video settings (Settings → Video, "Video Defaults", S5.8.1): whether video is decoded, going
//! fullscreen when a video starts, keeping the display awake, the picture adjustments, the
//! deinterlacing and the framing new media starts with.

use serde::{Deserialize, Serialize};

use super::super::lenient::lenient;
use crate::engine::media_options::{Deinterlace, DeinterlaceMethod, Picture};
use crate::video::framing::{AspectRatio, Crop};

/// Video preferences that follow the user between machines.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent switches, each its own row in Settings → Video"
)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoSettings {
    /// "Enable video": off, video tracks aren't decoded and media plays as sound only.
    #[serde(deserialize_with = "lenient")]
    pub enabled: bool,
    /// "Start videos in fullscreen": each new video goes fullscreen when its picture arrives.
    #[serde(deserialize_with = "lenient")]
    pub start_fullscreen: bool,
    /// "Stop the screensaver during playback": the display stays awake while video plays
    /// (S1.4.3).
    #[serde(deserialize_with = "lenient")]
    pub keep_display_awake: bool,
    /// "Picture": hue, brightness, contrast, saturation and gamma.
    #[serde(deserialize_with = "lenient")]
    pub picture: PictureSettings,
    /// "Deinterlacing": when.
    #[serde(deserialize_with = "lenient")]
    pub deinterlace: Deinterlace,
    /// "Method": how.
    #[serde(deserialize_with = "lenient")]
    pub deinterlace_method: DeinterlaceMethod,
    /// "Aspect ratio".
    #[serde(deserialize_with = "lenient")]
    pub aspect_ratio: AspectRatio,
    /// "Crop".
    #[serde(deserialize_with = "lenient")]
    pub crop: Crop,
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            start_fullscreen: false,
            keep_display_awake: true,
            picture: PictureSettings::default(),
            deinterlace: Deinterlace::default(),
            deinterlace_method: DeinterlaceMethod::default(),
            aspect_ratio: AspectRatio::default(),
            crop: Crop::default(),
        }
    }
}

/// "Picture", in libVLC's `adjust` units; the registry keeps each in range.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PictureSettings {
    /// "Hue", −180 to 180 degrees.
    #[serde(deserialize_with = "lenient")]
    pub hue: f32,
    /// "Brightness", 0 to 2.
    #[serde(deserialize_with = "lenient")]
    pub brightness: f32,
    /// "Contrast", 0 to 2.
    #[serde(deserialize_with = "lenient")]
    pub contrast: f32,
    /// "Saturation", 0 to 3.
    #[serde(deserialize_with = "lenient")]
    pub saturation: f32,
    /// "Gamma", 0.01 to 10.
    #[serde(deserialize_with = "lenient")]
    pub gamma: f32,
}

impl Default for PictureSettings {
    fn default() -> Self {
        Self::from(Picture::NEUTRAL)
    }
}

impl From<Picture> for PictureSettings {
    fn from(picture: Picture) -> Self {
        let Picture { hue, brightness, contrast, saturation, gamma } = picture;
        Self { hue, brightness, contrast, saturation, gamma }
    }
}

impl From<PictureSettings> for Picture {
    fn from(picture: PictureSettings) -> Self {
        let PictureSettings { hue, brightness, contrast, saturation, gamma } = picture;
        Self { hue, brightness, contrast, saturation, gamma }
    }
}
