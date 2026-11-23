//! Where full library mode's miniplayer card docks (S4.10.5), and where the Picture in Picture
//! window was left on each display (S8.5.2).

use serde::{Deserialize, Serialize};

use super::super::lenient::lenient;

/// The corner of the library the miniplayer card sits in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MiniplayerCorner {
    /// Top left.
    TopLeft,
    /// Top right.
    TopRight,
    /// Bottom left (`player_music` designs).
    BottomLeft,
    /// Bottom right, as most designs show it (`player_03`).
    #[default]
    BottomRight,
}

/// Where the Picture in Picture window was last left on one display.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PipPlace {
    /// The display's bounds, which name it: display ids don't outlive a launch.
    #[serde(deserialize_with = "lenient")]
    pub display: ScreenRect,
    /// The window's bounds on it.
    #[serde(deserialize_with = "lenient")]
    pub window: ScreenRect,
}

/// A rectangle in logical pixels, in GPUI's global coordinate space.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScreenRect {
    /// The left edge.
    #[serde(deserialize_with = "lenient")]
    pub x: f32,
    /// The top edge.
    #[serde(deserialize_with = "lenient")]
    pub y: f32,
    /// The width.
    #[serde(deserialize_with = "lenient")]
    pub width: f32,
    /// The height.
    #[serde(deserialize_with = "lenient")]
    pub height: f32,
}
