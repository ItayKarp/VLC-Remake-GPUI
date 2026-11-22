//! Where full library mode's miniplayer card docks (S4.10.5).

use serde::{Deserialize, Serialize};

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
