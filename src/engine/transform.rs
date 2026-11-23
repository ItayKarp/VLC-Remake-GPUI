//! Flipping and rotating the picture (S7.2.1, S7.2.2): what the user chose, and the one
//! libVLC `transform` filter type that does both.
//!
//! The picture is flipped first, then turned clockwise, both as it is shown. libVLC applies its
//! filter to the picture as stored, before turning it upright by the orientation the file
//! records, so for a file stored a quarter turn from upright the flips go along the other axis
//! ([`Transform::for_quarter_turned`]); turns simply add up.

use serde::{Deserialize, Serialize};

/// Degrees in a full turn, and in a quarter of one.
const FULL_TURN: f64 = 360.;
const QUARTER_TURN: f64 = 90.;

/// "Flip": mirroring the picture.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Flip {
    /// As it is.
    #[default]
    None,
    /// Left and right swapped.
    Horizontal,
    /// Top and bottom swapped.
    Vertical,
}

/// "Rotate": the picture turned clockwise in quarter turns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rotation {
    /// As it is.
    #[default]
    Upright,
    /// 90° clockwise.
    Quarter,
    /// 180°.
    Half,
    /// 270° clockwise (90° counterclockwise).
    ThreeQuarters,
}

impl Rotation {
    /// In order of the turns they make.
    const ALL: [Self; 4] = [Self::Upright, Self::Quarter, Self::Half, Self::ThreeQuarters];

    /// The clockwise angle, 0 to 270.
    pub fn degrees(self) -> u16 {
        match self {
            Self::Upright => 0,
            Self::Quarter => 90,
            Self::Half => 180,
            Self::ThreeQuarters => 270,
        }
    }

    /// The quarter turn nearest to `degrees` clockwise, any angle: −90 is 270, 440 is 90.
    pub fn nearest(degrees: f64) -> Self {
        let quarters = (degrees.rem_euclid(FULL_TURN) / QUARTER_TURN).round() as usize;
        Self::ALL[quarters % Self::ALL.len()]
    }

    /// Whether it turns the picture on its side, swapping its width and height.
    pub fn is_sideways(self) -> bool {
        matches!(self, Self::Quarter | Self::ThreeQuarters)
    }
}

/// A flip, then a rotation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Transform {
    /// The flip, done first.
    pub flip: Flip,
    /// The clockwise turn after it.
    pub rotation: Rotation,
}

impl Transform {
    /// The picture as it is.
    pub const NONE: Self = Self { flip: Flip::None, rotation: Rotation::Upright };

    /// libVLC's `transform-type` that flips and then turns as this says; `None` when the
    /// picture stays as it is.
    pub fn libvlc_type(self) -> Option<&'static str> {
        use {Flip as F, Rotation as R};
        // Flipping one way and then turning is a single mirror or turn: flipping left and right
        // then turning a quarter mirrors along the diagonal from top right to bottom left.
        Some(match (self.flip, self.rotation) {
            (F::None, R::Upright) => return None,
            (F::None, R::Quarter) => "90",
            (F::None, R::Half) => "180",
            (F::None, R::ThreeQuarters) => "270",
            (F::Horizontal, R::Upright) | (F::Vertical, R::Half) => "hflip",
            (F::Vertical, R::Upright) | (F::Horizontal, R::Half) => "vflip",
            (F::Horizontal, R::Quarter) | (F::Vertical, R::ThreeQuarters) => "antitranspose",
            (F::Vertical, R::Quarter) | (F::Horizontal, R::ThreeQuarters) => "transpose",
        })
    }

    /// Whether the shown picture has its width and height swapped.
    pub fn swaps_sides(self) -> bool {
        self.rotation.is_sideways()
    }

    /// This transform for a picture the file stores a quarter turn from upright, which libVLC
    /// turns upright after the filter: the flips go along the other axis.
    pub fn for_quarter_turned(self) -> Self {
        let flip = match self.flip {
            Flip::None => Flip::None,
            Flip::Horizontal => Flip::Vertical,
            Flip::Vertical => Flip::Horizontal,
        };
        Self { flip, ..self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transform(flip: Flip, rotation: Rotation) -> Transform {
        Transform { flip, rotation }
    }

    #[test]
    fn angles_snap_to_the_nearest_quarter_turn() {
        assert_eq!(Rotation::nearest(0.), Rotation::Upright);
        assert_eq!(Rotation::nearest(44.), Rotation::Upright);
        assert_eq!(Rotation::nearest(45.), Rotation::Quarter);
        assert_eq!(Rotation::nearest(-90.), Rotation::ThreeQuarters);
        assert_eq!(Rotation::nearest(440.), Rotation::Quarter);
        assert_eq!(Rotation::nearest(350.), Rotation::Upright);
        assert_eq!(Rotation::Half.degrees(), 180);
    }

    #[test]
    fn each_combination_is_one_libvlc_type() {
        assert_eq!(Transform::NONE.libvlc_type(), None);
        assert_eq!(transform(Flip::None, Rotation::Quarter).libvlc_type(), Some("90"));
        assert_eq!(transform(Flip::None, Rotation::Half).libvlc_type(), Some("180"));
        assert_eq!(transform(Flip::Horizontal, Rotation::Upright).libvlc_type(), Some("hflip"));
        assert_eq!(transform(Flip::Vertical, Rotation::Half).libvlc_type(), Some("hflip"));
        assert_eq!(transform(Flip::Horizontal, Rotation::Half).libvlc_type(), Some("vflip"));
        let diagonal = transform(Flip::Horizontal, Rotation::Quarter).libvlc_type();
        assert_eq!(diagonal, Some("antitranspose"));
        let diagonal = transform(Flip::Vertical, Rotation::Quarter).libvlc_type();
        assert_eq!(diagonal, Some("transpose"));
    }

    #[test]
    fn quarter_turned_files_flip_along_the_other_axis() {
        let mirrored = transform(Flip::Horizontal, Rotation::Half);
        assert_eq!(mirrored.for_quarter_turned(), transform(Flip::Vertical, Rotation::Half));
        assert!(transform(Flip::None, Rotation::ThreeQuarters).swaps_sides());
        assert!(!mirrored.swaps_sides());
    }
}
