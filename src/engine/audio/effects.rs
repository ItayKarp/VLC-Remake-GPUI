//! Settings → Audio's Loudness and Effects (S5.8.2) as the backend starts with them: libVLC 3
//! reads its audio filters and replay gain when its output is made, so they apply from the next
//! launch. The equalizer and its 2 Pass change live ([`super::equalizer`]).

use serde::{Deserialize, Serialize};

/// The audio filters and gain the backend starts with.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent effects, each its own row in Settings → Audio"
)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AudioEffects {
    /// "Normalize loudness" at this "Target level" (libVLC's `norm-max-level`); `None` is off.
    pub normalize: Option<f32>,
    /// "Night mode": a compressor that evens out loud and quiet passages.
    pub night_mode: bool,
    /// "Replay gain": which of the file's loudness tags evens out the volume.
    pub replay_gain: ReplayGain,
    /// "Replay gain preamp", in dB, added to the tagged gain.
    pub replay_gain_preamp: f32,
    /// "Spatial audio" in this mode; `None` is off.
    pub spatial: Option<SpatialMode>,
    /// "Stereo widening": makes stereo sound wider.
    pub stereo_widening: bool,
}

/// Which replay gain tags apply.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayGain {
    /// None: files play at their own level.
    #[default]
    Off,
    /// Each track's own tag, so every track sounds equally loud.
    Track,
    /// The album's tag, keeping the album's own loud and quiet tracks.
    Album,
}

impl ReplayGain {
    /// libVLC's `audio-replay-gain-mode` value.
    pub fn libvlc_name(self) -> &'static str {
        match self {
            Self::Off => "none",
            Self::Track => "track",
            Self::Album => "album",
        }
    }
}

/// How "Spatial audio" places sound around the listener.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpatialMode {
    /// Surround (5.1, 7.1) mixed for headphones as if from speakers around the listener.
    #[default]
    HeadphoneVirtualization,
    /// 360° (ambisonic) sound rendered binaurally for headphones.
    Binaural,
}
