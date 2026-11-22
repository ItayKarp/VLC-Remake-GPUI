//! Audio settings (S1.5, Settings → Audio, S5.8.2): whether audio plays, the volume range,
//! startup volume and step, the speaker setup and passthrough, loudness and effects, the
//! equalizer, the preferred languages (S1.6.3), and the machine-local output device and last
//! volume.

use serde::{Deserialize, Serialize};

use super::super::lenient::lenient;
use crate::engine::audio::SpeakerSetup;
use crate::engine::audio::effects::{ReplayGain, SpatialMode};
use crate::engine::audio::equalizer::{BANDS, Equalizer, Preset};

/// "Maximum volume" by default, in percent.
pub const DEFAULT_MAX_VOLUME_PERCENT: u32 = 125;
/// The "Always starts at" volume by default, in percent.
pub const DEFAULT_STARTUP_VOLUME_PERCENT: u32 = 100;
/// "Volume step" by default, in percent.
pub const DEFAULT_VOLUME_STEP_PERCENT: u32 = 5;
/// "Target level" by default: libVLC's `norm-max-level`.
pub const DEFAULT_LOUDNESS_TARGET: f32 = 2.;

/// Audio preferences that follow the user between machines.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent switches, each its own row in Settings → Audio"
)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    /// "Enable audio": off, audio tracks aren't decoded and media plays silently.
    #[serde(deserialize_with = "lenient")]
    pub enabled: bool,
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
    /// "Audio Language" (S7.6.1): language codes (`en`, `jpn`), most preferred first. New
    /// media plays the first one it has; empty keeps the media's own default.
    #[serde(deserialize_with = "lenient")]
    pub preferred_languages: Vec<String>,
    /// "Normalize loudness": evens out the volume to [`Self::loudness_target`]. Next launch.
    #[serde(deserialize_with = "lenient")]
    pub normalize_loudness: bool,
    /// "Target level", 0.5 to 10: louder passages than this are turned down.
    #[serde(deserialize_with = "lenient")]
    pub loudness_target: f32,
    /// "Night mode": a compressor that softens loud passages. Next launch.
    #[serde(deserialize_with = "lenient")]
    pub night_mode: bool,
    /// "Replay gain". Next launch.
    #[serde(deserialize_with = "lenient")]
    pub replay_gain: ReplayGain,
    /// "Replay gain preamp", in dB. Next launch.
    #[serde(deserialize_with = "lenient")]
    pub replay_gain_preamp: f32,
    /// "Equalizer": sound goes through the bands below.
    #[serde(deserialize_with = "lenient")]
    pub equalizer: bool,
    /// "Equalizer preset": the preset the bands were last set from; none once a band moves.
    #[serde(deserialize_with = "lenient")]
    pub equalizer_preset: Preset,
    /// The equalizer's preamp, in dB.
    #[serde(deserialize_with = "lenient")]
    pub equalizer_preamp: f32,
    /// The equalizer's bands, in dB, lowest frequency first.
    #[serde(deserialize_with = "lenient")]
    pub equalizer_bands: [f32; BANDS],
    /// "Two-pass": the equalizer runs twice, for a stronger effect. Next launch.
    #[serde(deserialize_with = "lenient")]
    pub equalizer_two_pass: bool,
    /// "Spatial audio", in [`Self::spatial_mode`]. Next launch.
    #[serde(deserialize_with = "lenient")]
    pub spatial_audio: bool,
    /// "Spatial audio mode".
    #[serde(deserialize_with = "lenient")]
    pub spatial_mode: SpatialMode,
    /// "Stereo widening". Next launch.
    #[serde(deserialize_with = "lenient")]
    pub stereo_widening: bool,
}

impl AudioSettings {
    /// The equalizer's gains as set.
    pub fn equalizer_gains(&self) -> Equalizer {
        Equalizer { preamp: self.equalizer_preamp, bands: self.equalizer_bands }
    }

    /// Sets the equalizer's gains; `preset` is the preset they came from, if any.
    pub fn set_equalizer_gains(&mut self, gains: Equalizer, preset: Preset) {
        self.equalizer_preamp = gains.preamp;
        self.equalizer_bands = gains.bands;
        self.equalizer_preset = preset;
    }
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            max_volume_percent: DEFAULT_MAX_VOLUME_PERCENT,
            startup_volume: StartupVolume::default(),
            startup_volume_percent: DEFAULT_STARTUP_VOLUME_PERCENT,
            volume_step_percent: DEFAULT_VOLUME_STEP_PERCENT,
            speaker_setup: SpeakerSetup::default(),
            passthrough: false,
            pause_on_device_disconnect: true,
            preferred_languages: Vec::new(),
            normalize_loudness: false,
            loudness_target: DEFAULT_LOUDNESS_TARGET,
            night_mode: false,
            replay_gain: ReplayGain::default(),
            replay_gain_preamp: 0.,
            equalizer: false,
            equalizer_preset: Preset::None,
            equalizer_preamp: Equalizer::FLAT.preamp,
            equalizer_bands: Equalizer::FLAT.bands,
            equalizer_two_pass: false,
            spatial_audio: false,
            spatial_mode: SpatialMode::default(),
            stereo_widening: false,
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
