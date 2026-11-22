//! Recording, casting and streaming-out settings (Settings → Recording, S5.10.3). The features
//! themselves are S8.6 (recording), S8.11 (casting) and S8.12 (streaming out); these are the
//! defaults they read. Where recordings go is a path on this machine, so it is local state.

use serde::{Deserialize, Serialize};

use super::super::lenient::lenient;

/// Recording, casting and streaming preferences (S5.10.3).
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent switches, each its own row in Settings → Recording"
)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RecordingSettings {
    /// "Record without re-encoding when possible": copies the streams as they are.
    #[serde(deserialize_with = "lenient")]
    pub keep_original_encoding: bool,
    /// "Discover cast devices": looks for Chromecasts on the local network.
    #[serde(deserialize_with = "lenient")]
    pub discover_cast_devices: bool,
    /// "Cast quality".
    #[serde(deserialize_with = "lenient")]
    pub cast_quality: CastQuality,
    /// "Play locally while streaming".
    #[serde(deserialize_with = "lenient")]
    pub play_locally_while_streaming: bool,
    /// "Include audio" in a stream.
    #[serde(deserialize_with = "lenient")]
    pub stream_audio: bool,
    /// "Include video" in a stream.
    #[serde(deserialize_with = "lenient")]
    pub stream_video: bool,
    /// "Include subtitles" in a stream.
    #[serde(deserialize_with = "lenient")]
    pub stream_subtitles: bool,
    /// "Encoding preset": x264's speed against size.
    #[serde(deserialize_with = "lenient")]
    pub encoding_preset: EncodingPreset,
    /// "Tuning": x264's tuning for the kind of picture.
    #[serde(deserialize_with = "lenient")]
    pub tuning: Tuning,
}

impl Default for RecordingSettings {
    fn default() -> Self {
        Self {
            keep_original_encoding: true,
            discover_cast_devices: true,
            cast_quality: CastQuality::default(),
            play_locally_while_streaming: true,
            stream_audio: true,
            stream_video: true,
            stream_subtitles: true,
            encoding_preset: EncodingPreset::default(),
            tuning: Tuning::default(),
        }
    }
}

/// "Cast quality": how media the cast device can't play is converted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CastQuality {
    /// Follows the network and the device.
    #[default]
    Automatic,
    /// Smallest, for a slow network.
    Low,
    /// Between the two.
    Medium,
    /// Best picture, for a fast network.
    High,
}

/// "Encoding preset": x264's presets, from fastest to smallest.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncodingPreset {
    /// The fastest, largest output.
    Ultrafast,
    /// Faster than usual.
    Fast,
    /// x264's own default.
    #[default]
    Medium,
    /// Smaller output, slower.
    Slow,
    /// The smallest output, slowest.
    Veryslow,
}

/// "Tuning": what x264 tunes for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tuning {
    /// No tuning.
    #[default]
    None,
    /// Live-action film.
    Film,
    /// Cartoons and anime.
    Animation,
    /// Keeps film grain.
    Grain,
    /// Slides and screen captures.
    StillImage,
    /// The least delay, for live streams.
    ZeroLatency,
}
