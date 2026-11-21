//! Pixel comparison of a rendered screen with its design.

use image::{Rgba, RgbaImage};

/// How far apart (0 to 255) a channel may be before its pixel counts as different.
/// Large enough for anti-aliasing and color-space rounding, small enough to see a
/// wrong color.
const CHANNEL_TOLERANCE: u8 = 40;

/// How much a matching pixel is darkened in the diff image, so red stands out.
const DIFF_DIM: u8 = 3;

/// The color of a mismatched pixel in the diff image.
const MISMATCH: Rgba<u8> = Rgba([255, 0, 0, 255]);

/// The result of comparing two images of the same size.
pub struct Comparison {
    /// The share of pixels (0 to 1) that differ.
    pub mismatch: f64,
    /// The actual image, dimmed, with mismatched pixels in red.
    pub diff: RgbaImage,
}

impl Comparison {
    /// Compares `actual` with `expected`, which must be the same size.
    pub fn new(actual: &RgbaImage, expected: &RgbaImage) -> Self {
        assert_eq!(actual.dimensions(), expected.dimensions(), "images to compare differ in size");
        let mut diff = RgbaImage::new(actual.width(), actual.height());
        let mut mismatched = 0_u64;
        for ((out, a), e) in diff.pixels_mut().zip(actual.pixels()).zip(expected.pixels()) {
            if differs(*a, *e) {
                mismatched += 1;
                *out = MISMATCH;
            } else {
                *out = dim(*a);
            }
        }
        let total = u64::from(actual.width()) * u64::from(actual.height());
        Self { mismatch: mismatched as f64 / total.max(1) as f64, diff }
    }
}

/// Whether any color channel is further apart than the tolerance. Alpha is ignored:
/// the designs have transparent rounded corners where the window is opaque.
fn differs(a: Rgba<u8>, b: Rgba<u8>) -> bool {
    a.0.iter().zip(b.0.iter()).take(3).any(|(x, y)| x.abs_diff(*y) > CHANNEL_TOLERANCE)
}

/// `pixel`, darkened and opaque.
fn dim(pixel: Rgba<u8>) -> Rgba<u8> {
    let [r, g, b, _] = pixel.0;
    Rgba([r / DIFF_DIM, g / DIFF_DIM, b / DIFF_DIM, 255])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_images_match() {
        let image = RgbaImage::from_pixel(4, 4, Rgba([10, 20, 30, 255]));
        assert!(Comparison::new(&image, &image).mismatch.abs() < f64::EPSILON);
    }

    #[test]
    fn small_channel_differences_are_tolerated() {
        let a = RgbaImage::from_pixel(2, 2, Rgba([100, 100, 100, 255]));
        let b = RgbaImage::from_pixel(2, 2, Rgba([100 + CHANNEL_TOLERANCE, 100, 100, 0]));
        assert!(Comparison::new(&a, &b).mismatch.abs() < f64::EPSILON);
    }

    #[test]
    fn a_wrong_color_counts_and_shows_red() {
        let a = RgbaImage::from_pixel(2, 1, Rgba([0, 0, 0, 255]));
        let mut b = a.clone();
        b.put_pixel(1, 0, Rgba([0, 200, 0, 255]));
        let comparison = Comparison::new(&a, &b);
        assert!((comparison.mismatch - 0.5).abs() < f64::EPSILON);
        assert_eq!(*comparison.diff.get_pixel(1, 0), MISMATCH);
    }
}
