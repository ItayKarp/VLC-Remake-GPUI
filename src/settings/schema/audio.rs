//! Audio settings (S1.5, Settings → Audio): the volume range, startup volume and step, the
//! speaker setup and passthrough, and the machine-local output device and last volume.

use serde::{Deserialize, Serialize};

use super::super::lenient::lenient;
use crate::engine::audio::SpeakerSetup;

/// "Maximum volume" by default, in percent.
pub const DEFAULT_MAX_VOLUME_PERCENT: u32 = 200;
/// The "Always starts at" volume by default, in percent.
pub const DEFAULT_STARTUP_VOLUME_PERCENT: u32 = 100;
/// "Volume step" by default, in percent.
pub const DEFAULT_VOLUME_STEP_PERCENT: u32 = 5;

/// Audio preferences that follow the user between machines.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    /// "Maximum volume", 100 to 300 (clamped where it is used).
    #[serde(deserialize_with = "lenient")]
    pub max_volume_percent: u32,
    /// "Startup volume": the last volume, or a fixed one.
    #[serde(deserialize_with = "lenient")]
    pub startup_volume: StartupVolume,
    /// "Always starts at", used with [`StartupVolume::Fixed`].
    #[serde(deserialize_with = "lenient")]
    pub startup_volume_percent: u32,
    /// "Volume step" for the Up/Down keys and the scroll wheel, 1 to 100.
    #[serde(deserialize_with = "lenient")]
    pub volume_step_percent: u32,
    /// "Speaker setup".
    #[serde(deserialize_with = "lenient")]
    pub speaker_setup: SpeakerSetup,
    /// Sends AC3 and DTS undecoded over HDMI or S/PDIF (advanced). Applies from the next
    /// launch.
    #[serde(deserialize_with = "lenient")]
    pub passthrough: bool,
    /// Pauses when the device that was playing disappears (headphones unplugged).
    #[serde(deserialize_with = "lenient")]
    pub pause_on_device_disconnect: bool,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            max_volume_percent: DEFAULT_MAX_VOLUME_PERCENT,
            startup_volume: StartupVolume::default(),
            startup_volume_percent: DEFAULT_STARTUP_VOLUME_PERCENT,
            volume_step_percent: DEFAULT_VOLUME_STEP_PERCENT,
            speaker_setup: SpeakerSetup::default(),
            passthrough: false,
            pause_on_device_disconnect: true,
        }
    }
}

/// The volume each launch starts at.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupVolume {
    /// "Remember last volume", but at most 100% (D21).
    #[default]
    RememberLast,
    /// "Always starts at" [`AudioSettings::startup_volume_percent`].
    Fixed,
}

/// Audio state that only makes sense on this machine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LocalAudio {
    /// The chosen output device's id; empty for the system default.
    #[serde(deserialize_with = "lenient")]
    pub output_device: String,
    /// The volume when the app last changed it, in percent.
    #[serde(deserialize_with = "lenient")]
    pub last_volume_percent: u32,
}

impl Default for LocalAudio {
    fn default() -> Self {
        Self { output_device: String::new(), last_volume_percent: DEFAULT_STARTUP_VOLUME_PERCENT }
    }
}
