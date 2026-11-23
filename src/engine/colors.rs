//! The color filters (S7.4): libVLC's `extract`, `colorthres`, `invert`, `posterize`, `gradient`
//! and `sepia`, each off unless [`Colors`] names it. Like the picture's adjustments, the player
//! applies them live, to the media playing and the media after it.
//!
//! Video passes through them in one fixed order, the panel's from top to bottom, so the two that
//! pick a color see it before the others change it; they come after the picture's adjustments,
//! sharpening and banding removal, and before its film grain, which lies over everything as on
//! film (`engine/libvlc/cpu_filters.rs`).

use std::ffi::CStr;

use serde::{Deserialize, Serialize};

/// The color filters to show video through, in libVLC's own units.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Colors {
    /// Color Extraction: keeps this color (`0xRRGGBB`) and turns the rest grey.
    pub extract: Option<u32>,
    /// Color Threshold: keeps the colors near one, greying the rest.
    pub threshold: Option<ColorThreshold>,
    /// Negate Colors: inverts every color.
    pub negate: bool,
    /// Posterize: this many levels per channel, 2 to 256.
    pub posterize: Option<u32>,
    /// Gradient: draws the picture's gradients or edges.
    pub gradient: Option<Gradient>,
    /// Sepia: this intensity, 1 to 255.
    pub sepia: Option<u32>,
}

impl Colors {
    /// Video as decoded: every filter off.
    pub const NONE: Self = Self {
        extract: None,
        threshold: None,
        negate: false,
        posterize: None,
        gradient: None,
        sepia: None,
    };

    /// Whether every filter is off.
    pub fn is_none(&self) -> bool {
        *self == Self::NONE
    }
}

/// Color Threshold's color and how near another must be to keep its color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorThreshold {
    /// The color kept, `0xRRGGBB`.
    pub color: u32,
    /// The least saturation a pixel needs to keep its color, 0 to 255.
    pub saturation: u32,
    /// How far from `color` a pixel's color may be, 0 to 255.
    pub similarity: u32,
}

/// How Gradient draws the picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gradient {
    /// What it draws.
    pub mode: GradientMode,
    /// Whether it keeps the picture's colors rather than drawing in white.
    pub colored: bool,
    /// Whether it draws the picture as a cartoon (Gradient and Edge only).
    pub cartoon: bool,
}

/// What Gradient draws: libVLC's `gradient-mode` values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GradientMode {
    /// How fast brightness changes, everywhere.
    #[default]
    Gradient,
    /// Only the edges.
    Edge,
    /// Circles, found with a Hough transform; slow.
    Hough,
}

impl GradientMode {
    /// libVLC's name for the mode.
    pub fn libvlc_name(self) -> &'static CStr {
        match self {
            Self::Gradient => c"gradient",
            Self::Edge => c"edge",
            Self::Hough => c"hough",
        }
    }
}
