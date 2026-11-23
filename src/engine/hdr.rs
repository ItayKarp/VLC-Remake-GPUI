//! HDR video (S7.5): how an HDR picture is shown, and what the engine learned about the video.
//!
//! libVLC 3 hands `vmem` 10-bit PQ and HLG pictures converted to 8 bits without tone mapping, so
//! they look dim and washed out (ADR 0001, limit 5). The engine tone-maps them itself instead:
//! it asks for the 10-bit picture and maps it to SDR on the CPU ([`ToneMappingSink`]), which
//! needs software decoding, as the CPU filters do. Every display gets SDR: GPUI draws 8-bit
//! surfaces, so HDR is never passed through, even to an HDR display.
//!
//! What the choices mean (S7.5.2): they say how the source's picture is read, since the output
//! is always SDR.
//! - [`HdrMode::Auto`] follows the video's own metadata: PQ (HDR10, and HDR10+ and Dolby Vision
//!   8.1 by their HDR10 base) or HLG is tone-mapped, anything else shown as it is.
//! - [`HdrMode::Off`] maps nothing: HDR shows as libVLC decodes it, washed out, but keeps
//!   hardware decoding.
//! - [`HdrMode::Hdr10`] and [`HdrMode::Hlg`] read the picture as PQ or HLG whatever its metadata
//!   says, for files tagged wrongly or not at all.
//!
//! HDR10+ and Dolby Vision aren't offered: libVLC 3 passes neither HDR10+'s dynamic metadata nor
//! Dolby Vision's reshaping layer, so forcing them could only mean HDR10.

mod convert;
mod sink;
mod tone_map;

use serde::{Deserialize, Serialize};

pub use sink::ToneMappingSink;

/// The brightness SDR's white stands for, in nits (ITU-R BT.2408's HDR reference white).
pub const REFERENCE_WHITE: f32 = 203.;
/// The brightest HDR10 video is assumed to get when its metadata doesn't say, in nits, and HLG's
/// nominal peak.
pub const DEFAULT_PEAK: f32 = 1000.;
/// PQ's brightest value, in nits.
pub const PQ_PEAK: f32 = 10_000.;

/// How HDR video is shown ("HDR" in the Video tab and Settings → Video).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HdrMode {
    /// Tone-map what the video's metadata says is HDR.
    #[default]
    Auto,
    /// Show HDR as decoded, without tone mapping.
    Off,
    /// Read the picture as PQ (HDR10) and tone-map it.
    Hdr10,
    /// Read the picture as HLG and tone-map it.
    Hlg,
}

impl HdrMode {
    /// How to tone-map video of `colour`; `None` shows it as decoded.
    pub fn tone_map(self, colour: SourceColour) -> Option<ToneMap> {
        let transfer = match self {
            Self::Off => return None,
            Self::Auto => colour.transfer?,
            Self::Hdr10 => Transfer::Pq,
            Self::Hlg => Transfer::Hlg,
        };
        Some(ToneMap { transfer, peak: colour.peak_for(transfer) })
    }
}

/// An HDR transfer function: how the picture's values stand for brightness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transfer {
    /// SMPTE ST 2084, "PQ": absolute brightness up to 10,000 nits (HDR10, HDR10+, Dolby Vision).
    Pq,
    /// ARIB STD-B67, Hybrid Log-Gamma: brightness relative to the display's.
    Hlg,
}

/// What the video's metadata says about its colors, as far as libVLC 3 tells.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SourceColour {
    /// Its HDR transfer function; `None` for SDR, or when the metadata doesn't say.
    pub transfer: Option<Transfer>,
    /// Its brightest pixel (MaxCLL), else its mastering display's peak, in nits.
    pub peak: Option<f32>,
    /// Whether it is Dolby Vision.
    pub dolby_vision: bool,
}

impl SourceColour {
    /// The peak to tone-map from when the picture is read as `transfer`.
    fn peak_for(self, transfer: Transfer) -> f32 {
        match transfer {
            Transfer::Pq => {
                self.peak.map_or(DEFAULT_PEAK, |peak| peak.clamp(REFERENCE_WHITE, PQ_PEAK))
            }
            Transfer::Hlg => DEFAULT_PEAK,
        }
    }

    /// Dolby Vision without a PQ or HLG base layer: profile 5, whose IPT colors libVLC 3 can't
    /// convert, so it shows purple and green.
    pub fn is_unshowable_dolby_vision(self) -> bool {
        self.dolby_vision && self.transfer.is_none()
    }
}

/// How an HDR picture is tone-mapped to SDR.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToneMap {
    /// How its values are read.
    pub transfer: Transfer,
    /// Its brightest value, in nits, which is mapped to SDR's white.
    pub peak: f32,
}

/// What the engine knows about the video playing, for the player to show.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HdrStatus {
    /// The video's colors, once read; `None` while unknown or without video.
    pub source: Option<SourceColour>,
    /// How it is tone-mapped now; `None` while it shows as decoded.
    pub tone_map: Option<ToneMap>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const HDR10: SourceColour =
        SourceColour { transfer: Some(Transfer::Pq), peak: Some(400.), dolby_vision: false };

    #[test]
    fn auto_follows_the_metadata() {
        let map = HdrMode::Auto.tone_map(HDR10);
        assert_eq!(map, Some(ToneMap { transfer: Transfer::Pq, peak: 400. }));
        assert_eq!(HdrMode::Auto.tone_map(SourceColour::default()), None);
        let hlg = SourceColour { transfer: Some(Transfer::Hlg), ..SourceColour::default() };
        assert_eq!(HdrMode::Auto.tone_map(hlg).map(|map| map.peak), Some(DEFAULT_PEAK));
    }

    #[test]
    fn off_maps_nothing_and_the_others_force_a_transfer() {
        assert_eq!(HdrMode::Off.tone_map(HDR10), None);
        let forced = HdrMode::Hdr10.tone_map(SourceColour::default());
        assert_eq!(forced, Some(ToneMap { transfer: Transfer::Pq, peak: DEFAULT_PEAK }));
        assert_eq!(HdrMode::Hlg.tone_map(HDR10).map(|map| map.transfer), Some(Transfer::Hlg));
    }

    #[test]
    fn peaks_stay_between_sdr_white_and_pqs_limit() {
        let dim = SourceColour { peak: Some(50.), ..HDR10 };
        assert_eq!(HdrMode::Auto.tone_map(dim).map(|map| map.peak), Some(REFERENCE_WHITE));
        let bright = SourceColour { peak: Some(40_000.), ..HDR10 };
        assert_eq!(HdrMode::Auto.tone_map(bright).map(|map| map.peak), Some(PQ_PEAK));
    }

    #[test]
    fn dolby_vision_without_a_base_layer_is_unshowable() {
        let profile5 = SourceColour { dolby_vision: true, ..SourceColour::default() };
        assert!(profile5.is_unshowable_dolby_vision());
        assert!(!SourceColour { dolby_vision: true, ..HDR10 }.is_unshowable_dolby_vision());
    }
}
