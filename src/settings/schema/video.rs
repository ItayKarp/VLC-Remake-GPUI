//! Video settings (Settings → Video, "Video Defaults", S5.8.1): whether video is decoded, going
//! fullscreen when a video starts, keeping the display awake, the picture adjustments, the
//! deinterlacing, the flip and rotation, the framing (S7.2), the color filters (S7.4) and how
//! HDR shows (S7.5) every video shows with, and what snapshots are saved as (S8.4).

use serde::{Deserialize, Serialize};

use super::super::lenient::lenient;
use super::video_colors::ColorSettings;
use crate::engine::hdr::HdrMode;
use crate::engine::media_options::{Deinterlace, DeinterlaceMethod, Deinterlacing, Picture};
use crate::engine::transform::{Flip, Rotation, Transform};
use crate::snapshot::SnapshotFormat;
use crate::video::framing::{AspectRatio, Crop, CropEdges, Zoom};

/// The Custom aspect ratio until the user types one: ultrawide cinema.
const DEFAULT_CUSTOM_RATIO: &str = "2.4:1";

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
    /// "Picture": hue, brightness, contrast, saturation and gamma, sharpening, banding removal and
    /// film grain.
    #[serde(deserialize_with = "lenient")]
    pub picture: PictureSettings,
    /// "Colors": the color filters.
    #[serde(deserialize_with = "lenient")]
    pub colors: ColorSettings,
    /// "Deinterlacing": when.
    #[serde(deserialize_with = "lenient")]
    pub deinterlace: Deinterlace,
    /// "Method": how.
    #[serde(deserialize_with = "lenient")]
    pub deinterlace_method: DeinterlaceMethod,
    /// "Aspect ratio".
    #[serde(deserialize_with = "lenient")]
    pub aspect_ratio: AspectRatio,
    /// "Custom ratio": the aspect ratio's Custom shape, as typed ("64:27").
    #[serde(deserialize_with = "lenient")]
    pub custom_aspect_ratio: String,
    /// "Crop".
    #[serde(deserialize_with = "lenient")]
    pub crop: Crop,
    /// "Trim edges" and its pixels: the crop panel's Top, Bottom, Left and Right (S7.2.5).
    #[serde(deserialize_with = "lenient")]
    pub crop_edges: CropEdges,
    /// "Zoom".
    #[serde(deserialize_with = "lenient")]
    pub zoom: Zoom,
    /// "Flip".
    #[serde(deserialize_with = "lenient")]
    pub flip: Flip,
    /// "Rotate".
    #[serde(deserialize_with = "lenient")]
    pub rotation: Rotation,
    /// "HDR".
    #[serde(deserialize_with = "lenient")]
    pub hdr: HdrMode,
    /// "Snapshot format" (S8.4).
    #[serde(deserialize_with = "lenient")]
    pub snapshot_format: SnapshotFormat,
}

impl VideoSettings {
    /// The flip and rotation.
    pub fn transform(&self) -> Transform {
        Transform { flip: self.flip, rotation: self.rotation }
    }

    /// The deinterlacing.
    pub fn deinterlacing(&self) -> Deinterlacing {
        Deinterlacing { mode: self.deinterlace, method: self.deinterlace_method }
    }
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            start_fullscreen: false,
            keep_display_awake: true,
            picture: PictureSettings::default(),
            colors: ColorSettings::default(),
            deinterlace: Deinterlace::default(),
            deinterlace_method: DeinterlaceMethod::default(),
            aspect_ratio: AspectRatio::default(),
            custom_aspect_ratio: DEFAULT_CUSTOM_RATIO.to_owned(),
            crop: Crop::default(),
            crop_edges: CropEdges::default(),
            zoom: Zoom::default(),
            flip: Flip::default(),
            rotation: Rotation::default(),
            hdr: HdrMode::default(),
            snapshot_format: SnapshotFormat::default(),
        }
    }
}

/// "Picture", in libVLC's units (its `adjust`, `sharpen`, `gradfun` and `grain` filters'); the
/// registry keeps each in range.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PictureSettings {
    /// "Adjust the picture": off, the values below are kept but the picture shows as the media
    /// has it. Off by default, as in VLC.
    #[serde(deserialize_with = "lenient")]
    pub enabled: bool,
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
    /// "Sharpen Sigma", 0 to 2; 0 is off.
    #[serde(deserialize_with = "lenient")]
    pub sharpen: f32,
    /// "Banding Removal Radius", 4 to 32 pixels; 0 is off.
    #[serde(deserialize_with = "lenient")]
    pub deband_radius: u32,
    /// "Film Grain Variance", 0 to 10; 0 is off.
    #[serde(deserialize_with = "lenient")]
    pub grain: f32,
}

impl PictureSettings {
    /// The adjustments to show the picture with: `None` while off, or while they change nothing.
    pub fn applied(&self) -> Option<Picture> {
        let PictureSettings {
            enabled,
            hue,
            brightness,
            contrast,
            saturation,
            gamma,
            sharpen,
            deband_radius,
            grain,
        } = *self;
        let picture =
            Picture { hue, brightness, contrast, saturation, gamma, sharpen, deband_radius, grain };
        (enabled && !picture.is_neutral()).then_some(picture)
    }
}

impl Default for PictureSettings {
    fn default() -> Self {
        let Picture { hue, brightness, contrast, saturation, gamma, sharpen, deband_radius, grain } =
            Picture::NEUTRAL;
        Self {
            enabled: false,
            hue,
            brightness,
            contrast,
            saturation,
            gamma,
            sharpen,
            deband_radius,
            grain,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_picture_applies_only_while_on_and_changed() {
        let mut picture = PictureSettings { brightness: 1.5, ..PictureSettings::default() };
        assert_eq!(picture.applied(), None, "off by default");
        picture.enabled = true;
        assert_eq!(picture.applied(), Some(Picture { brightness: 1.5, ..Picture::NEUTRAL }));
        let neutral = PictureSettings { enabled: true, ..PictureSettings::default() };
        assert_eq!(neutral.applied(), None, "no filter for a picture it wouldn't change");
        let grainy = PictureSettings { enabled: true, grain: 2., ..PictureSettings::default() };
        assert_eq!(grainy.applied(), Some(Picture { grain: 2., ..Picture::NEUTRAL }));
    }
}
