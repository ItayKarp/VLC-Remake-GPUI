//! The 10-band equalizer (Settings → Audio's Effects, S5.8.2; the panel's Equalizer, S7.7): its
//! bands, the range they move in, and VLC's presets, which `libvlc/equalizer.rs` checks against
//! libVLC's own.

use serde::{Deserialize, Serialize};

/// The number of bands.
pub const BANDS: usize = 10;
/// The lowest and highest gain of a band and of the preamp, in dB.
pub const GAIN_RANGE: (f32, f32) = (-20., 20.);
/// Each band's middle frequency, as the panels label it (libVLC's ISO bands; the design's "KHz"
/// is a typo).
pub const FREQUENCIES: [&str; BANDS] = [
    "60 Hz", "170 Hz", "310 Hz", "600 Hz", "1 kHz", "3 kHz", "6 kHz", "12 kHz", "14 kHz", "16 kHz",
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

    /// The gains kept within [`GAIN_RANGE`], a gain that isn't a number at 0 dB: for gains read
    /// from a hand-edited or older settings file.
    #[must_use]
    pub fn clamped(self) -> Self {
        Self { preamp: clamp_gain(self.preamp), bands: self.bands.map(clamp_gain) }
    }

    /// The preamp, then each band, lowest frequency first: the eleven gains the panels show.
    pub fn gains(&self) -> impl Iterator<Item = f32> + use<> {
        std::iter::once(self.preamp).chain(self.bands)
    }

    /// The equalizer with gain `index` (0 the preamp, then the bands) set to `gain`, clamped;
    /// an index past the last band changes nothing.
    #[must_use]
    pub fn with_gain(mut self, index: usize, gain: f32) -> Self {
        match index.checked_sub(1) {
            None => self.preamp = clamp_gain(gain),
            Some(band) if band < BANDS => self.bands[band] = clamp_gain(gain),
            Some(_) => {}
        }
        self
    }

    /// The equalizer whose gains are `gains` (the preamp, then the bands), clamped; `None`
    /// unless there are exactly eleven.
    pub fn from_gains(gains: &[f32]) -> Option<Self> {
        let (&preamp, bands) = gains.split_first()?;
        let bands: [f32; BANDS] = bands.try_into().ok()?;
        Some(Self { preamp, bands }.clamped())
    }
}

/// A gain as the panels show it: "12.0 dB", never "-0.0 dB".
pub fn format_gain(gain: f64) -> String {
    let tenths = (gain * 10.).round() / 10.;
    let gain = crate::i18n::decimal(if tenths == 0. { 0. } else { tenths }, 1);
    tr!("{gain} dB", gain)
}

/// A typed gain: a number in dB, "dB" after it or not, with a point or a comma; kept within
/// [`GAIN_RANGE`]. Anything else is refused with the reason.
pub fn parse_gain(text: &str) -> Result<f64, &'static str> {
    let text = text.trim();
    let number = text
        .strip_suffix("dB")
        .or_else(|| text.strip_suffix("db"))
        .or_else(|| text.strip_suffix("DB"))
        .unwrap_or(text)
        .trim()
        .replace(',', ".")
        .replace('\u{2212}', "-");
    match number.parse::<f64>() {
        Ok(gain) if gain.is_finite() => {
            Ok(gain.clamp(f64::from(GAIN_RANGE.0), f64::from(GAIN_RANGE.1)))
        }
        _ => Err(tr!("A gain from -20 to 20 dB")),
    }
}

/// A gain within [`GAIN_RANGE`]; one that isn't a number is 0 dB.
fn clamp_gain(gain: f32) -> f32 {
    if gain.is_nan() { 0. } else { gain.clamp(GAIN_RANGE.0, GAIN_RANGE.1) }
}

/// How the equalizer plays: its gains, and whether sound goes through it twice ("2 Pass",
/// S7.7.5), for a stronger effect.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Equalization {
    /// The gains.
    pub gains: Equalizer,
    /// Runs the bands twice.
    pub two_pass: bool,
}

impl Default for Equalizer {
    fn default() -> Self {
        Self::FLAT
    }
}

/// Where the equalizer's gains came from: one of VLC's presets, or the user's own ("Custom",
/// S7.7.4), set band by band or from a preset they saved.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs, reason = "each preset is named for the music it suits")]
pub enum Preset {
    /// The bands as the user set them. Settings files before S7.7 called it "none".
    #[serde(alias = "none")]
    Custom,
    #[default]
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

    /// The preset's id in settings files and the presets dropdown.
    pub fn id(self) -> &'static str {
        self.names().0
    }

    /// The preset's name ("Full bass & treble"; the design's "Full base" is a typo).
    pub fn label(self) -> &'static str {
        crate::i18n::text(self.names().1)
    }

    /// The preset whose id is `id`.
    pub fn from_id(id: &str) -> Option<Self> {
        std::iter::once(Self::Custom).chain(Self::ALL).find(|preset| preset.id() == id)
    }

    fn names(self) -> (&'static str, &'static str) {
        match self {
            Self::Custom => ("custom", tr_noop!("Custom")),
            Self::Flat => ("flat", tr_noop!("Flat")),
            Self::Classical => ("classical", tr_noop!("Classical")),
            Self::Club => ("club", tr_noop!("Club")),
            Self::Dance => ("dance", tr_noop!("Dance")),
            Self::FullBass => ("full_bass", tr_noop!("Full bass")),
            Self::FullBassAndTreble => ("full_bass_and_treble", tr_noop!("Full bass & treble")),
            Self::FullTreble => ("full_treble", tr_noop!("Full treble")),
            Self::Headphones => ("headphones", tr_noop!("Headphones")),
            Self::LargeHall => ("large_hall", tr_noop!("Large Hall")),
            Self::Live => ("live", tr_noop!("Live")),
            Self::Party => ("party", tr_noop!("Party")),
            Self::Pop => ("pop", tr_noop!("Pop")),
            Self::Reggae => ("reggae", tr_noop!("Reggae")),
            Self::Rock => ("rock", tr_noop!("Rock")),
            Self::Ska => ("ska", tr_noop!("Ska")),
            Self::Soft => ("soft", tr_noop!("Soft")),
            Self::SoftRock => ("soft_rock", tr_noop!("Soft rock")),
            Self::Techno => ("techno", tr_noop!("Techno")),
        }
    }

    /// The preset's gains, as libVLC 3 defines them; `None` for Custom.
    pub fn equalizer(self) -> Option<Equalizer> {
        let (preamp, bands) = match self {
            Self::Custom => return None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gains_out_of_range_are_clamped() {
        let wild = Equalizer { preamp: 99., bands: [f32::NAN; BANDS] }.clamped();
        assert_eq!(wild.preamp, 20.);
        assert_eq!(wild.bands, [0.; BANDS]);
        assert_eq!(Equalizer::FLAT.with_gain(3, -50.).bands[2], -20.);
        assert_eq!(Equalizer::FLAT.with_gain(11, 5.), Equalizer::FLAT);
    }

    #[test]
    fn gains_read_and_show_in_tenths_of_a_decibel() {
        assert_eq!(format_gain(12.), "12.0 dB");
        assert_eq!(format_gain(-0.01), "0.0 dB");
        assert_eq!(parse_gain(" -4,5 dB"), Ok(-4.5));
        assert_eq!(parse_gain("+3db"), Ok(3.));
        assert_eq!(parse_gain("40"), Ok(20.));
        assert!(parse_gain("loud").is_err());
        assert!(parse_gain("").is_err());
    }

    #[test]
    fn eleven_gains_make_an_equalizer() {
        let gains: Vec<f32> = Equalizer::FLAT.gains().collect();
        assert_eq!(Equalizer::from_gains(&gains), Some(Equalizer::FLAT));
        assert_eq!(Equalizer::from_gains(&gains[1..]), None);
    }

    #[test]
    fn presets_read_back_by_id_and_old_files_mean_custom() {
        for preset in Preset::ALL {
            assert_eq!(Preset::from_id(preset.id()), Some(preset));
        }
        assert_eq!(Preset::from_id("custom"), Some(Preset::Custom));
        let old: Preset = serde_json::from_str("\"none\"").expect("an old preset reads");
        assert_eq!(old, Preset::Custom);
        assert_eq!(Preset::FullBassAndTreble.label(), "Full bass & treble");
    }
}
