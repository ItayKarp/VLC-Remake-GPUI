//! The audio effects after the equalizer (S7.9), as Settings → Audio and the player's settings
//! panel show them: the Compressor in libVLC's units, the Spatializer on a 0–10 scale (libVLC's
//! 0–1), and the Stereo Widener's gains as real percentages (libVLC's fractions; the design's
//! "0.2 %" is a fraction labelled as a percentage). Each is off by default, as in VLC.

use serde::{Deserialize, Serialize};

use super::super::lenient::lenient;
use super::audio::AudioSettings;
use crate::engine::audio::effects::{AudioEffects, Compressor, Spatializer, StereoWidener};

/// The Spatializer's display scale over libVLC's 0–1.
const SPATIALIZER_SCALE: f32 = 10.;
/// The Stereo Widener's gains are percentages of libVLC's fractions.
const PERCENT: f32 = 100.;

/// "Compressor": a dynamic range compressor, in libVLC's units.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CompressorSettings {
    /// Sound goes through it.
    #[serde(deserialize_with = "lenient")]
    pub enabled: bool,
    /// "RMS / Peak", 0 (peak) to 1 (RMS).
    #[serde(deserialize_with = "lenient")]
    pub rms_peak: f32,
    /// "Attack", 1.5 to 400 ms.
    #[serde(deserialize_with = "lenient")]
    pub attack_ms: f32,
    /// "Release", 2 to 800 ms.
    #[serde(deserialize_with = "lenient")]
    pub release_ms: f32,
    /// "Threshold", −30 to 0 dB.
    #[serde(deserialize_with = "lenient")]
    pub threshold_db: f32,
    /// "Ratio", 1 : 1 to 20 : 1.
    #[serde(deserialize_with = "lenient")]
    pub ratio: f32,
    /// "Knee Radius", 1 to 10 dB.
    #[serde(deserialize_with = "lenient")]
    pub knee_db: f32,
    /// "Makeup Gain", 0 to 24 dB.
    #[serde(deserialize_with = "lenient")]
    pub makeup_db: f32,
}

impl Default for CompressorSettings {
    fn default() -> Self {
        let c = Compressor::DEFAULT;
        Self {
            enabled: false,
            rms_peak: c.rms_peak,
            attack_ms: c.attack_ms,
            release_ms: c.release_ms,
            threshold_db: c.threshold_db,
            ratio: c.ratio,
            knee_db: c.knee_db,
            makeup_db: c.makeup_db,
        }
    }
}

impl CompressorSettings {
    /// The compressor these values make, within libVLC's ranges.
    fn compressor(&self) -> Compressor {
        Compressor {
            rms_peak: self.rms_peak,
            attack_ms: self.attack_ms,
            release_ms: self.release_ms,
            threshold_db: self.threshold_db,
            ratio: self.ratio,
            knee_db: self.knee_db,
            makeup_db: self.makeup_db,
        }
        .clamped()
    }
}

/// "Spatializer": a room's reverberation around the sound, each value 0 to 10.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SpatializerSettings {
    /// Sound goes through it.
    #[serde(deserialize_with = "lenient")]
    pub enabled: bool,
    /// "Size": the room's.
    #[serde(deserialize_with = "lenient")]
    pub size: f32,
    /// "Width": the reverberation's spread.
    #[serde(deserialize_with = "lenient")]
    pub width: f32,
    /// "Wet": the reverberation's level.
    #[serde(deserialize_with = "lenient")]
    pub wet: f32,
    /// "Dry": the original sound's level.
    #[serde(deserialize_with = "lenient")]
    pub dry: f32,
    /// "Damp": how much the walls soften high frequencies.
    #[serde(deserialize_with = "lenient")]
    pub damp: f32,
}

impl Default for SpatializerSettings {
    fn default() -> Self {
        let s = Spatializer::DEFAULT;
        Self {
            enabled: false,
            size: scaled(s.size, SPATIALIZER_SCALE),
            width: scaled(s.width, SPATIALIZER_SCALE),
            wet: scaled(s.wet, SPATIALIZER_SCALE),
            dry: scaled(s.dry, SPATIALIZER_SCALE),
            damp: scaled(s.damp, SPATIALIZER_SCALE),
        }
    }
}

impl SpatializerSettings {
    /// The spatializer these values make, on libVLC's 0–1 scale.
    fn spatializer(&self) -> Spatializer {
        let unit = |value: f32| value / SPATIALIZER_SCALE;
        Spatializer {
            size: unit(self.size),
            width: unit(self.width),
            wet: unit(self.wet),
            dry: unit(self.dry),
            damp: unit(self.damp),
        }
        .clamped()
    }
}

/// "Stereo widening" and the Stereo Widener's values, its gains in percent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StereoWidenerSettings {
    /// Sound goes through it.
    #[serde(deserialize_with = "lenient")]
    pub enabled: bool,
    /// "Delay Time", 1 to 100 ms.
    #[serde(deserialize_with = "lenient")]
    pub delay_ms: f32,
    /// "Feedback Gain", 0 to 90%.
    #[serde(deserialize_with = "lenient")]
    pub feedback_percent: f32,
    /// "Crossfeed", 0 to 80%.
    #[serde(deserialize_with = "lenient")]
    pub crossfeed_percent: f32,
    /// "Dry Mix", 0 to 100%.
    #[serde(deserialize_with = "lenient")]
    pub dry_mix_percent: f32,
}

impl Default for StereoWidenerSettings {
    fn default() -> Self {
        let w = StereoWidener::DEFAULT;
        Self {
            enabled: false,
            delay_ms: w.delay_ms,
            feedback_percent: scaled(w.feedback, PERCENT),
            crossfeed_percent: scaled(w.crossfeed, PERCENT),
            dry_mix_percent: scaled(w.dry_mix, PERCENT),
        }
    }
}

impl StereoWidenerSettings {
    /// The widener these values make, its gains as libVLC's fractions.
    fn widener(&self) -> StereoWidener {
        StereoWidener {
            delay_ms: self.delay_ms,
            feedback: self.feedback_percent / PERCENT,
            crossfeed: self.crossfeed_percent / PERCENT,
            dry_mix: self.dry_mix_percent / PERCENT,
        }
        .clamped()
    }
}

/// `value` times `by`, to a tenth: 0.3 × 100 is 30, not 30.000002.
fn scaled(value: f32, by: f32) -> f32 {
    (value * by * 10.).round() / 10.
}

impl AudioSettings {
    /// The effects after the equalizer these settings turn on. The Compressor's section wins
    /// over Night mode, which is a compressor preset ([`Compressor::NIGHT`]).
    pub fn effects(&self) -> AudioEffects {
        let compressor = if self.compressor.enabled {
            Some(self.compressor.compressor())
        } else {
            self.night_mode.then_some(Compressor::NIGHT)
        };
        AudioEffects {
            compressor,
            spatializer: self.spatializer.enabled.then(|| self.spatializer.spatializer()),
            widener: self.stereo_widener.enabled.then(|| self.stereo_widener.widener()),
            spatial: self.spatial_audio.then_some(self.spatial_mode),
            normalize: self.normalize_loudness.then_some(self.loudness_target),
            replay_gain: self.replay_gain,
            replay_gain_preamp: self.replay_gain_preamp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_on_by_default() {
        let effects = AudioSettings::default().effects();
        assert_eq!(effects, AudioEffects::default());
    }

    #[test]
    fn the_defaults_are_libvlcs_in_display_units() {
        let mut audio = AudioSettings::default();
        audio.compressor.enabled = true;
        audio.spatializer.enabled = true;
        audio.stereo_widener.enabled = true;
        let effects = audio.effects();
        assert_eq!(effects.compressor, Some(Compressor::DEFAULT));
        assert_eq!(effects.spatializer, Some(Spatializer::DEFAULT));
        assert_eq!(effects.widener, Some(StereoWidener::DEFAULT));
        assert!((audio.stereo_widener.feedback_percent - 30.).abs() < 1e-4);
    }

    #[test]
    fn the_compressor_section_wins_over_night_mode() {
        let mut audio = AudioSettings { night_mode: true, ..AudioSettings::default() };
        assert_eq!(audio.effects().compressor, Some(Compressor::NIGHT));
        audio.compressor.enabled = true;
        audio.compressor.ratio = 50.;
        let compressor = audio.effects().compressor.expect("on");
        assert!((compressor.ratio - 20.).abs() < f32::EPSILON, "clamped to libVLC's range");
    }
}
