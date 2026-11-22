//! The 10-band equalizer (Settings → Audio's Effects, S5.8.2): its bands, the range they move in,
//! and VLC's presets, which `libvlc/tests.rs` checks against libVLC's own.

use serde::{Deserialize, Serialize};

/// The number of bands.
pub const BANDS: usize = 10;
/// The lowest and highest gain of a band and of the preamp, in dB.
pub const GAIN_RANGE: (f32, f32) = (-20., 20.);
/// Each band's middle frequency, as the panel labels it (libVLC's ISO bands).
pub const FREQUENCIES: [&str; BANDS] = [
    "60 Hz", "170 Hz", "310 Hz", "600 Hz", "1 KHz", "3 KHz", "6 KHz", "12 KHz", "14 KHz", "16 KHz",
];

/// An equalizer's gains, in dB.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Equalizer {
    /// The gain before the bands.
    pub preamp: f32,
    /// Each band's gain, lowest frequency first.
    pub bands: [f32; BANDS],
}

impl Equalizer {
    /// VLC's Flat preset: a 12 dB preamp with every band at 0.
    pub const FLAT: Self = Self { preamp: 12., bands: [0.; BANDS] };
}

impl Default for Equalizer {
    fn default() -> Self {
        Self::FLAT
    }
}

/// "Equalizer preset": none, so the bands are as the user set them, or one of VLC's presets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs, reason = "each preset is named for the music it suits")]
pub enum Preset {
    /// No preset: the bands as set.
    #[default]
    None,
    Flat,
    Classical,
    Club,
    Dance,
    FullBass,
    FullBassAndTreble,
    FullTreble,
    Headphones,
    LargeHall,
    Live,
    Party,
    Pop,
    Reggae,
    Rock,
    Ska,
    Soft,
    SoftRock,
    Techno,
}

impl Preset {
    /// VLC's presets, in libVLC's order: a preset's position is its libVLC index.
    pub const ALL: [Self; 18] = [
        Self::Flat,
        Self::Classical,
        Self::Club,
        Self::Dance,
        Self::FullBass,
        Self::FullBassAndTreble,
        Self::FullTreble,
        Self::Headphones,
        Self::LargeHall,
        Self::Live,
        Self::Party,
        Self::Pop,
        Self::Reggae,
        Self::Rock,
        Self::Ska,
        Self::Soft,
        Self::SoftRock,
        Self::Techno,
    ];

    /// The preset's gains, as libVLC 3 defines them; `None` for no preset.
    pub fn equalizer(self) -> Option<Equalizer> {
        let (preamp, bands) = match self {
            Self::None => return None,
            Self::Flat => (12., [0.; BANDS]),
            Self::Classical => (12., [0., 0., 0., 0., 0., 0., -7.2, -7.2, -7.2, -9.6]),
            Self::Club => (6., [0., 0., 8., 5.6, 5.6, 5.6, 3.2, 0., 0., 0.]),
            Self::Dance => (5., [9.6, 7.2, 2.4, 0., 0., -5.6, -7.2, -7.2, 0., 0.]),
            Self::FullBass => (5., [-8., 9.6, 9.6, 5.6, 1.6, -4., -8., -10.4, -11.2, -11.2]),
            Self::FullBassAndTreble => (4., [7.2, 5.6, 0., -7.2, -4.8, 1.6, 8., 11.2, 12., 12.]),
            Self::FullTreble => (3., [-9.6, -9.6, -9.6, -4., 2.4, 11.2, 16., 16., 16., 16.8]),
            Self::Headphones => (4., [4.8, 11.2, 5.6, -3.2, -2.4, 1.6, 4.8, 9.6, 12.8, 14.4]),
            Self::LargeHall => (5., [10.4, 10.4, 5.6, 5.6, 0., -4.8, -4.8, -4.8, 0., 0.]),
            Self::Live => (7., [-4.8, 0., 4., 5.6, 5.6, 5.6, 4., 2.4, 2.4, 2.4]),
            Self::Party => (6., [7.2, 7.2, 0., 0., 0., 0., 0., 0., 7.2, 7.2]),
            Self::Pop => (6., [-1.6, 4.8, 7.2, 8., 5.6, 0., -2.4, -2.4, -1.6, -1.6]),
            Self::Reggae => (8., [0., 0., 0., -5.6, 0., 6.4, 6.4, 0., 0., 0.]),
            Self::Rock => (5., [8., 4.8, -5.6, -8., -3.2, 4., 8.8, 11.2, 11.2, 11.2]),
            Self::Ska => (6., [-2.4, -4.8, -4., 0., 4., 5.6, 8.8, 9.6, 11.2, 9.6]),
            Self::Soft => (5., [4.8, 1.6, 0., -2.4, 0., 4., 8., 9.6, 11.2, 12.]),
            Self::SoftRock => (7., [4., 4., 2.4, 0., -4., -5.6, -3.2, 0., 2.4, 8.8]),
            Self::Techno => (5., [8., 5.6, 0., -5.6, -4.8, 0., 8., 9.6, 9.6, 8.8]),
        };
        Some(Equalizer { preamp, bands })
    }
}
