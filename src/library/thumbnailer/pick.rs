//! Choosing a video's thumbnail frame (S4.4.1): where to look, and which pictures are worth
//! keeping. About 15% into the video skips most intros and title cards; a black or flat picture
//! (a fade, a slate) is passed over for the next time to try.

use std::time::Duration;

use crate::engine::thumbnail::Frame;

/// Where to look, as percentages of the length, in order.
const TRY_PERCENT: [u32; 4] = [15, 25, 10, 40];
/// Where to look when the length isn't known.
const TRY_UNKNOWN: [Duration; 3] =
    [Duration::from_secs(30), Duration::from_secs(10), Duration::from_secs(2)];
/// A picture whose mean brightness (0–255) is below this is black.
const DARK_BRIGHTNESS: f64 = 24.;
/// A picture whose brightness varies less than this (standard deviation) is flat.
const FLAT_DEVIATION: f64 = 10.;
/// Brightness is measured on every this-many-th pixel: plenty for a mean.
const SAMPLE_EVERY: usize = 7;
/// Bytes per BGRA pixel.
const BGRA: usize = 4;

/// The times to try, in order, for a video of `length`.
pub fn times(length: Option<Duration>) -> Vec<Duration> {
    match length.filter(|length| !length.is_zero()) {
        Some(length) => TRY_PERCENT.iter().map(|&percent| length * percent / 100).collect(),
        None => TRY_UNKNOWN.to_vec(),
    }
}

/// How a picture looks: its brightness and how much it varies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// Mean brightness, 0–255.
    pub brightness: f64,
    /// Standard deviation of the brightness.
    pub deviation: f64,
}

impl Look {
    /// Measures `frame`.
    pub fn of(frame: &Frame) -> Self {
        let samples: Vec<f64> = frame
            .bgra
            .as_chunks::<BGRA>()
            .0
            .iter()
            .step_by(SAMPLE_EVERY)
            .map(|pixel| {
                let [blue, green, red] = [pixel[0], pixel[1], pixel[2]].map(f64::from);
                // Rec. 601 luma.
                0.114f64.mul_add(blue, 0.587f64.mul_add(green, 0.299 * red))
            })
            .collect();
        if samples.is_empty() {
            return Self { brightness: 0., deviation: 0. };
        }
        let count = samples.len() as f64;
        let brightness = samples.iter().sum::<f64>() / count;
        let variance =
            samples.iter().map(|sample| (sample - brightness).powi(2)).sum::<f64>() / count;
        Self { brightness, deviation: variance.sqrt() }
    }

    /// Whether the picture shows something: neither black nor flat.
    pub fn is_usable(self) -> bool {
        self.brightness >= DARK_BRIGHTNESS && self.deviation >= FLAT_DEVIATION
    }
}

/// The best picture offered so far: the first usable one, else the most varied.
#[derive(Default)]
pub struct Best {
    best: Option<(Frame, f64)>,
}

impl Best {
    /// Offers `frame`; `true` when it is usable, so nothing more needs trying.
    pub fn offer(&mut self, frame: Frame) -> bool {
        let look = Look::of(&frame);
        if look.is_usable() {
            self.best = Some((frame, f64::INFINITY));
            return true;
        }
        if self.best.as_ref().is_none_or(|(_, deviation)| look.deviation > *deviation) {
            self.best = Some((frame, look.deviation));
        }
        false
    }

    /// The picture to keep, if any was offered.
    pub fn into_frame(self) -> Option<Frame> {
        self.best.map(|(frame, _)| frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(value: u8) -> Frame {
        Frame { width: 16, height: 16, bgra: [value, value, value, 255].repeat(256) }
    }

    fn striped() -> Frame {
        let bgra = (0..256).flat_map(|i| if i % 2 == 0 { [0; 4] } else { [255; 4] }).collect();
        Frame { width: 16, height: 16, bgra }
    }

    #[test]
    fn times_start_near_15_percent() {
        let times = times(Some(Duration::from_secs(1000)));
        assert_eq!(times[0], Duration::from_secs(150));
        assert_eq!(times.len(), TRY_PERCENT.len());
        assert_eq!(super::times(Some(Duration::ZERO)), TRY_UNKNOWN);
        assert_eq!(super::times(None), TRY_UNKNOWN);
    }

    #[test]
    fn black_and_flat_pictures_are_not_usable() {
        assert!(!Look::of(&flat(0)).is_usable());
        assert!(!Look::of(&flat(128)).is_usable());
        assert!(Look::of(&striped()).is_usable());
        assert_eq!(Look::of(&Frame { width: 0, height: 0, bgra: vec![] }).brightness, 0.);
    }

    #[test]
    fn the_first_usable_picture_wins_else_the_most_varied() {
        let mut best = Best::default();
        assert!(!best.offer(flat(0)));
        assert!(best.offer(striped()));
        assert_eq!(best.into_frame(), Some(striped()));
        let mut dark = Best::default();
        assert!(!dark.offer(flat(5)));
        assert_eq!(dark.into_frame(), Some(flat(5)));
        assert_eq!(Best::default().into_frame(), None);
    }
}
