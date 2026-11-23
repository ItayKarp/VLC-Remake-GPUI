//! The crop panel's edges (S7.2.5): pixels trimmed off each side of the picture.
//!
//! They count the video's own pixels, not the screen's, as it arrives: square, and already
//! turned if the file or the user turns it, so Top is the top of the picture as shown. However
//! much is asked, at least [`MIN_KEPT`] pixels of the picture stay each way.

use serde::{Deserialize, Serialize};

use crate::settings::lenient::lenient;

/// The fewest video pixels a crop leaves across and down.
pub const MIN_KEPT: u32 = 16;

/// Pixels trimmed off each side, while on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CropEdges {
    /// Whether the edges are trimmed; off, the numbers are kept but the picture is whole.
    #[serde(deserialize_with = "lenient")]
    pub enabled: bool,
    /// Off the top.
    #[serde(deserialize_with = "lenient")]
    pub top: u32,
    /// Off the bottom.
    #[serde(deserialize_with = "lenient")]
    pub bottom: u32,
    /// Off the left.
    #[serde(deserialize_with = "lenient")]
    pub left: u32,
    /// Off the right.
    #[serde(deserialize_with = "lenient")]
    pub right: u32,
}

/// The shares of the picture trimmed off each side, each 0 to 1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Trim {
    /// Off the top.
    pub top: f32,
    /// Off the bottom.
    pub bottom: f32,
    /// Off the left.
    pub left: f32,
    /// Off the right.
    pub right: f32,
}

impl Trim {
    /// The share of the width kept.
    pub fn kept_width(self) -> f32 {
        1. - self.left - self.right
    }

    /// The share of the height kept.
    pub fn kept_height(self) -> f32 {
        1. - self.top - self.bottom
    }
}

impl CropEdges {
    /// What is trimmed off a picture of `pixels` (width, height): nothing while off, and each
    /// pair shrunk alike when it would leave less than [`MIN_KEPT`].
    pub fn trim(self, pixels: (u32, u32)) -> Trim {
        if !self.enabled {
            return Trim::default();
        }
        let (left, right) = pair(self.left, self.right, pixels.0);
        let (top, bottom) = pair(self.top, self.bottom, pixels.1);
        Trim { top, bottom, left, right }
    }

    /// The most two opposite sides may trim together off `side` pixels.
    pub fn most(side: u32) -> u32 {
        side.saturating_sub(MIN_KEPT)
    }
}

/// Two opposite edges of a `side` pixels long as shares of it, shrunk alike to fit.
fn pair(first: u32, second: u32, side: u32) -> (f32, f32) {
    let wanted = first.saturating_add(second);
    let most = CropEdges::most(side);
    if side == 0 || wanted == 0 || most == 0 {
        return (0., 0.);
    }
    let shrink = if wanted > most { most as f32 / wanted as f32 } else { 1. };
    let share = |edge: u32| edge as f32 * shrink / side as f32;
    (share(first), share(second))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edges(top: u32, bottom: u32, left: u32, right: u32) -> CropEdges {
        CropEdges { enabled: true, top, bottom, left, right }
    }

    #[test]
    fn edges_are_shares_of_the_picture() {
        let trim = edges(108, 0, 192, 192).trim((1920, 1080));
        assert!((trim.top - 0.1).abs() < 1e-6);
        assert!((trim.kept_width() - 0.8).abs() < 1e-6);
        assert_eq!(trim.bottom, 0.);
    }

    #[test]
    fn at_least_sixteen_pixels_stay() {
        let trim = edges(0, 0, 1000, 1000).trim((1920, 1080));
        assert!((trim.kept_width() * 1920. - MIN_KEPT as f32).abs() < 1e-3);
        assert!((trim.left - trim.right).abs() < 1e-6, "both shrink alike");
        assert_eq!(edges(0, 0, 4, 4).trim((10, 10)), Trim::default(), "too small to trim");
    }

    #[test]
    fn off_trims_nothing() {
        let off = CropEdges { enabled: false, ..edges(10, 10, 10, 10) };
        assert_eq!(off.trim((1920, 1080)), Trim::default());
    }
}
