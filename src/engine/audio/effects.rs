//! The audio effects after the equalizer (S5.8.2, S7.9), in libVLC's units: the compressor, the
//! spatializer, the stereo widener, headphone virtualization and binaural rendering, then the
//! loudness normalizer and replay gain. They change what plays at once (`libvlc/audio_effects.rs`);
//! sound goes through them in a fixed order: equalizer → compressor → spatializer → stereo
//! widener → headphone virtualization → normalizer → volume.

use std::ffi::CStr;

use serde::{Deserialize, Serialize};

/// The audio effects in use; each `None` is off.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AudioEffects {
    /// The compressor: the panel's section, else Night mode's preset ([`Compressor::NIGHT`]).
    pub compressor: Option<Compressor>,
    /// The spatializer: a room's reverberation around the sound.
    pub spatializer: Option<Spatializer>,
    /// The stereo widener.
    pub widener: Option<StereoWidener>,
    /// "Spatial audio" in this mode.
    pub spatial: Option<SpatialMode>,
    /// "Normalize loudness" at this "Target level" (libVLC's `norm-max-level`).
    pub normalize: Option<f32>,
    /// "Replay gain": which of the file's loudness tags evens out the volume.
    pub replay_gain: ReplayGain,
    /// "Replay gain preamp", in dB, added to the tagged gain.
    pub replay_gain_preamp: f32,
}

/// libVLC's dynamic range compressor (`compressor`), each value within its range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Compressor {
    /// "RMS / Peak": how the level is measured, from 0 (peak) to 1 (RMS).
    pub rms_peak: f32,
    /// "Attack", in ms: how fast it turns loud sound down.
    pub attack_ms: f32,
    /// "Release", in ms: how fast it lets go.
    pub release_ms: f32,
    /// "Threshold", in dB: the level above which sound is turned down.
    pub threshold_db: f32,
    /// "Ratio", n : 1: how much sound over the threshold is turned down.
    pub ratio: f32,
    /// "Knee Radius", in dB: how gently it starts around the threshold.
    pub knee_db: f32,
    /// "Makeup Gain", in dB: added after, to win back the level.
    pub makeup_db: f32,
}

impl Compressor {
    /// libVLC's defaults, which the design shows.
    pub const DEFAULT: Self = Self {
        rms_peak: 0.2,
        attack_ms: 25.,
        release_ms: 100.,
        threshold_db: -11.,
        ratio: 4.,
        knee_db: 5.,
        makeup_db: 7.,
    };

    /// "Night mode": loud passages well down and quiet ones lifted, measured over RMS so speech
    /// isn't pumped, for listening at low volume. A signal at 0 dB comes out near −8 dB, one at
    /// −30 dB near −18 dB.
    pub const NIGHT: Self = Self {
        rms_peak: 1.,
        attack_ms: 10.,
        release_ms: 300.,
        threshold_db: -24.,
        ratio: 6.,
        knee_db: 6.,
        makeup_db: 12.,
    };

    /// libVLC's range for each value: RMS / peak, attack, release, threshold, ratio, knee and
    /// makeup gain.
    pub const RANGES: [(f32, f32); 7] =
        [(0., 1.), (1.5, 400.), (2., 800.), (-30., 0.), (1., 20.), (1., 10.), (0., 24.)];

    /// Each value kept within libVLC's range.
    pub fn clamped(self) -> Self {
        let [rms, attack, release, threshold, ratio, knee, makeup] = Self::RANGES;
        Self {
            rms_peak: clamp(self.rms_peak, rms),
            attack_ms: clamp(self.attack_ms, attack),
            release_ms: clamp(self.release_ms, release),
            threshold_db: clamp(self.threshold_db, threshold),
            ratio: clamp(self.ratio, ratio),
            knee_db: clamp(self.knee_db, knee),
            makeup_db: clamp(self.makeup_db, makeup),
        }
    }
}

/// libVLC's spatializer (`spatializer`, a Freeverb room), each value from 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spatializer {
    /// "Size": the room's size.
    pub size: f32,
    /// "Width": how wide the reverberation spreads.
    pub width: f32,
    /// "Wet": the reverberation's level.
    pub wet: f32,
    /// "Dry": the original sound's level.
    pub dry: f32,
    /// "Damp": how much the walls soften high frequencies.
    pub damp: f32,
}

impl Spatializer {
    /// libVLC's defaults, but the size, which the design shows at 0.8 (libVLC's 0.85).
    pub const DEFAULT: Self = Self { size: 0.8, width: 1., wet: 0.4, dry: 0.5, damp: 0.5 };

    /// Each value kept within 0 to 1.
    pub fn clamped(self) -> Self {
        let unit = |value: f32| clamp(value, (0., 1.));
        Self {
            size: unit(self.size),
            width: unit(self.width),
            wet: unit(self.wet),
            dry: unit(self.dry),
            damp: unit(self.damp),
        }
    }
}

/// libVLC's stereo widener (`stereo_widen`): a delayed, crossfed copy of each channel added to
/// the other. Its gains are fractions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StereoWidener {
    /// "Delay Time", in ms.
    pub delay_ms: f32,
    /// "Feedback Gain", 0 to 0.9.
    pub feedback: f32,
    /// "Crossfeed", 0 to 0.8.
    pub crossfeed: f32,
    /// "Dry Mix", 0 to 1.
    pub dry_mix: f32,
}

impl StereoWidener {
    /// libVLC's defaults.
    pub const DEFAULT: Self = Self { delay_ms: 20., feedback: 0.3, crossfeed: 0.3, dry_mix: 0.8 };

    /// libVLC's range for each value: delay, feedback, crossfeed and dry mix.
    pub const RANGES: [(f32, f32); 4] = [(1., 100.), (0., 0.9), (0., 0.8), (0., 1.)];

    /// Each value kept within libVLC's range.
    pub fn clamped(self) -> Self {
        let [delay, feedback, crossfeed, dry] = Self::RANGES;
        Self {
            delay_ms: clamp(self.delay_ms, delay),
            feedback: clamp(self.feedback, feedback),
            crossfeed: clamp(self.crossfeed, crossfeed),
            dry_mix: clamp(self.dry_mix, dry),
        }
    }
}

/// `value` within `range`; a value that isn't a number becomes the low end.
fn clamp(value: f32, (low, high): (f32, f32)) -> f32 {
    if value.is_nan() { low } else { value.clamp(low, high) }
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
    pub fn libvlc_name(self) -> &'static CStr {
        match self {
            Self::Off => c"none",
            Self::Track => c"track",
            Self::Album => c"album",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_presets_are_within_range() {
        for compressor in [Compressor::DEFAULT, Compressor::NIGHT] {
            assert_eq!(compressor.clamped(), compressor);
        }
        assert_eq!(Spatializer::DEFAULT.clamped(), Spatializer::DEFAULT);
        assert_eq!(StereoWidener::DEFAULT.clamped(), StereoWidener::DEFAULT);
    }

    #[test]
    fn wild_values_are_clamped() {
        let wild = Compressor { ratio: 99., attack_ms: f32::NAN, ..Compressor::DEFAULT }.clamped();
        assert!((wild.ratio - 20.).abs() < f32::EPSILON);
        assert!((wild.attack_ms - 1.5).abs() < f32::EPSILON);
        let widener = StereoWidener { feedback: 1., ..StereoWidener::DEFAULT }.clamped();
        assert!((widener.feedback - 0.9).abs() < f32::EPSILON);
    }
}
