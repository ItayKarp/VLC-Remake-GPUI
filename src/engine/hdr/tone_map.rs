//! The tone-mapping math: one pixel of a 10-bit BT.2020 PQ or HLG picture to SDR BT.709.
//!
//! Each pixel goes from Y'CbCr to R'G'B', through the transfer function to light (in units of
//! SDR's white, [`REFERENCE_WHITE`]), from BT.2020's primaries to BT.709's, and through a curve
//! that leaves everything up to [`KNEE`] alone and rolls the rest off so the peak lands on white.
//! The curve scales all three channels by what it does to the brightest, so hues stay as they
//! were. Then it is gamma-encoded for an SDR display. The transfer functions and the gamma are
//! tables, so a pixel costs a few multiplications and lookups.

use super::{REFERENCE_WHITE, ToneMap, Transfer};

/// Entries in each table.
const TABLE_SIZE: usize = 4096;
/// The light below which the curve leaves the picture alone, in units of SDR's white.
const KNEE: f32 = 0.75;
/// 10-bit limited range: black's and white's luma, the chroma's zero and its span.
const LUMA_BLACK: f32 = 64.;
const LUMA_SPAN: f32 = 876.;
const CHROMA_ZERO: f32 = 512.;
const CHROMA_SPAN: f32 = 896.;
/// BT.2020's (non-constant luminance) Y'CbCr to R'G'B' coefficients.
const CR_TO_R: f32 = 1.4746;
const CB_TO_G: f32 = -0.164_553;
const CR_TO_G: f32 = -0.571_353;
const CB_TO_B: f32 = 1.8814;
/// BT.2020's luminance weights, for HLG's system gamma.
const LUMINANCE_2020: [f32; 3] = [0.2627, 0.6780, 0.0593];
/// Light in BT.2020's primaries to BT.709's (ITU-R BT.2087).
const BT2020_TO_BT709: [[f32; 3]; 3] =
    [[1.6605, -0.5876, -0.0728], [-0.1246, 1.1329, -0.0083], [-0.0182, -0.1006, 1.1187]];
/// SMPTE ST 2084's constants.
const PQ_M1: f64 = 2610. / 16384.;
const PQ_M2: f64 = 2523. / 4096. * 128.;
const PQ_C1: f64 = 3424. / 4096.;
const PQ_C2: f64 = 2413. / 4096. * 32.;
const PQ_C3: f64 = 2392. / 4096. * 32.;
/// ARIB STD-B67's constants, its nominal display peak in nits and its system gamma there.
const HLG_A: f64 = 0.178_832_77;
const HLG_B: f64 = 0.284_668_92;
const HLG_C: f64 = 0.559_910_73;
const HLG_PEAK: f32 = 1000.;
const HLG_GAMMA: f64 = 1.2;
/// The SDR display's gamma.
const DISPLAY_GAMMA: f64 = 2.2;

/// Maps pixels as one [`ToneMap`] says.
pub struct Mapper {
    map: ToneMap,
    /// Light for an R', G' or B' value, by [`index`].
    to_light: Vec<f32>,
    /// HLG's system gamma, `Ys^(γ−1)`, by [`root_index`] of the scene luminance `Ys`.
    hlg_gain: Option<Vec<f32>>,
    /// The gamma-encoded value of light by [`root_index`].
    encode: Vec<f32>,
    /// The peak, in units of SDR's white, measured from the knee.
    peak_over_knee: f32,
}

impl Mapper {
    /// A mapper for `map`.
    pub fn new(map: ToneMap) -> Self {
        let to_light = table(|signal| match map.transfer {
            Transfer::Pq => pq_to_nits(signal) / f64::from(REFERENCE_WHITE),
            Transfer::Hlg => hlg_to_scene(signal),
        });
        let hlg_gain = (map.transfer == Transfer::Hlg).then(|| {
            let scale = f64::from(HLG_PEAK / REFERENCE_WHITE);
            table(|root| scale * (root * root).powf(HLG_GAMMA - 1.))
        });
        let encode = table(|root| (root * root).powf(1. / DISPLAY_GAMMA));
        let peak = (map.peak / REFERENCE_WHITE).max(1.);
        Self { map, to_light, hlg_gain, encode, peak_over_knee: (peak - KNEE) / (1. - KNEE) }
    }

    /// The tone map this mapper follows.
    pub fn tone_map(&self) -> ToneMap {
        self.map
    }

    /// What a pixel's 10-bit chroma adds to its R', G' and B'.
    #[inline]
    pub fn chroma(cb: u16, cr: u16) -> [f32; 3] {
        let cb = (f32::from(cb) - CHROMA_ZERO) / CHROMA_SPAN;
        let cr = (f32::from(cr) - CHROMA_ZERO) / CHROMA_SPAN;
        [CR_TO_R * cr, CB_TO_G * cb + CR_TO_G * cr, CB_TO_B * cb]
    }

    /// The gamma-encoded SDR R'G'B' (0 to 1) of a pixel with 10-bit luma `y` and `chroma`.
    #[inline]
    pub fn pixel(&self, y: u16, chroma: [f32; 3]) -> [f32; 3] {
        let luma = (f32::from(y) - LUMA_BLACK) / LUMA_SPAN;
        let light = chroma.map(|offset| self.to_light[index(luma + offset)]);
        let light = self.display_light(light);
        let [r, g, b] = BT2020_TO_BT709.map(|row| dot(row, light).max(0.));
        let brightest = r.max(g).max(b);
        let scale = if brightest > KNEE { self.curve(brightest) / brightest } else { 1. };
        [r, g, b].map(|channel| self.encode[root_index(channel * scale)])
    }

    /// HLG's light is the scene's: its system gamma turns it into the display's.
    #[inline]
    fn display_light(&self, light: [f32; 3]) -> [f32; 3] {
        let Some(gain) = &self.hlg_gain else { return light };
        let gain = gain[root_index(dot(LUMINANCE_2020, light))];
        light.map(|channel| channel * gain)
    }

    /// The tone curve above the knee: an extended Reinhard curve from the knee, reaching white
    /// at the peak, with the slope 1 at the knee so nothing bends there.
    #[inline]
    fn curve(&self, light: f32) -> f32 {
        let over = (light - KNEE) / (1. - KNEE);
        let peak = self.peak_over_knee;
        let rolled = over * (1. + over / (peak * peak)) / (1. + over);
        (KNEE + (1. - KNEE) * rolled).min(1.)
    }
}

/// A table of `f` at `TABLE_SIZE` points from 0 to 1.
fn table(f: impl Fn(f64) -> f64) -> Vec<f32> {
    (0..TABLE_SIZE).map(|i| f(i as f64 / (TABLE_SIZE - 1) as f64) as f32).collect()
}

/// The table entry for a value from 0 to 1, clamped.
#[inline]
fn index(value: f32) -> usize {
    (value.clamp(0., 1.) * (TABLE_SIZE - 1) as f32 + 0.5) as usize
}

/// The table entry for light from 0 to 1 by its square root, which spaces the entries closer
/// in the shadows, where the eye sees steps.
#[inline]
fn root_index(light: f32) -> usize {
    index(light.max(0.).sqrt())
}

#[inline]
fn dot(row: [f32; 3], light: [f32; 3]) -> f32 {
    row[0] * light[0] + row[1] * light[1] + row[2] * light[2]
}

/// SMPTE ST 2084's EOTF: a PQ value (0 to 1) as nits.
fn pq_to_nits(signal: f64) -> f64 {
    let power = signal.powf(1. / PQ_M2);
    let light = ((power - PQ_C1).max(0.) / (PQ_C2 - PQ_C3 * power)).powf(1. / PQ_M1);
    light * f64::from(super::PQ_PEAK)
}

/// ARIB STD-B67's inverse OETF: an HLG value (0 to 1) as scene light (0 to 1).
fn hlg_to_scene(signal: f64) -> f64 {
    if signal <= 0.5 {
        signal * signal / 3.
    } else {
        (((signal - HLG_C) / HLG_A).exp() + HLG_B) / 12.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 10-bit limited-range grey of this PQ or HLG value.
    fn grey(signal: f32) -> u16 {
        (LUMA_BLACK + signal * LUMA_SPAN).round() as u16
    }

    fn mapper(transfer: Transfer, peak: f32) -> Mapper {
        Mapper::new(ToneMap { transfer, peak })
    }

    #[test]
    fn pq_reads_absolute_brightness() {
        assert!((pq_to_nits(1.) - 10_000.).abs() < 1.);
        assert!((pq_to_nits(0.508) - 100.).abs() < 1.);
        assert_eq!(pq_to_nits(0.), 0.);
    }

    #[test]
    fn hlg_is_continuous_at_its_join() {
        let below = hlg_to_scene(0.5);
        let above = hlg_to_scene(0.500_001);
        assert!((below - above).abs() < 1e-4);
        assert!((hlg_to_scene(1.) - 1.).abs() < 1e-3);
    }

    #[test]
    fn black_stays_black_and_the_peak_reaches_white() {
        let mapper = mapper(Transfer::Pq, 1000.);
        assert_eq!(mapper.pixel(grey(0.), Mapper::chroma(512, 512)), [0.; 3]);
        // 1000 nits in PQ is 0.7518.
        let white = mapper.pixel(grey(0.7518), Mapper::chroma(512, 512));
        assert!(white.iter().all(|channel| *channel > 0.99), "{white:?}");
    }

    #[test]
    fn reference_white_shows_near_sdr_white_and_greys_stay_grey() {
        let mapper = mapper(Transfer::Pq, 1000.);
        // 203 nits in PQ is 0.5806.
        let [r, g, b] = mapper.pixel(grey(0.5806), Mapper::chroma(512, 512));
        assert!((0.85..0.99).contains(&r), "{r}");
        assert!((r - g).abs() < 0.01 && (g - b).abs() < 0.01);
    }

    #[test]
    fn hlg_reference_white_shows_near_sdr_white() {
        // HLG's reference white is 75%.
        let [r, ..] = mapper(Transfer::Hlg, 1000.).pixel(grey(0.75), Mapper::chroma(512, 512));
        assert!((0.85..0.99).contains(&r), "{r}");
    }

    #[test]
    fn the_curve_is_smooth_at_the_knee_and_rises_to_white() {
        let mapper = mapper(Transfer::Pq, 4000.);
        assert!((mapper.curve(KNEE + 1e-4) - KNEE).abs() < 1e-3);
        assert!(mapper.curve(2.) < mapper.curve(4.));
        assert!((mapper.curve(4000. / REFERENCE_WHITE) - 1.).abs() < 1e-4);
    }

    #[test]
    fn saturated_red_stays_red() {
        let mapper = mapper(Transfer::Pq, 1000.);
        // A strong BT.2020 red: high Cr, low Cb.
        let [r, g, b] = mapper.pixel(grey(0.45), Mapper::chroma(400, 900));
        assert!(r > 0.5 && r > 3. * g && r > 3. * b, "{r} {g} {b}");
    }
}
