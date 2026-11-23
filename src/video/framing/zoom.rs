//! "Zoom" (S7.2.3): how large the picture is drawn in the video area.
//!
//! A picture larger than the area is centered and clipped to it; it can't be dragged around.
//! Panning would take the drag the video area uses to move the window, and a zoom past the
//! window is rare enough that Fill and the window's own size cover it.

use serde::{Deserialize, Serialize};

/// How large the picture is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Zoom {
    /// As large as fits, bars where the shapes differ.
    #[default]
    Fit,
    /// As small as covers the area, the overflow cut off.
    Fill,
    /// A video pixel to each screen pixel.
    Original,
    /// Half that.
    Half,
    /// Twice that.
    Double,
}

impl Zoom {
    /// Screen pixels per video pixel; `None` when the size follows the area.
    pub fn factor(self) -> Option<f32> {
        match self {
            Self::Fit | Self::Fill => None,
            Self::Original => Some(1.),
            Self::Half => Some(0.5),
            Self::Double => Some(2.),
        }
    }
}
