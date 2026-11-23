//! The player layout Customize Player saves (S8.7), as item ids; `player_layout` reads and
//! writes it. It roams with the user, like the rest of the interface.

use serde::{Deserialize, Serialize};

use super::super::lenient::lenient;

/// What the title bar and the control bar show, by item id (`menu`, `play-pause`, `timeline`,
/// `spacer`, `gap-16`, …). No rows means the designed layout, so a later release's default
/// reaches users who never customized it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StoredLayout {
    /// The title bar's ids after the logo, left to right.
    #[serde(deserialize_with = "lenient")]
    pub title_bar: Vec<String>,
    /// The control bar's rows, top to bottom, each its ids left to right. Empty while the
    /// layout is the designed one.
    #[serde(deserialize_with = "lenient")]
    pub controls: Vec<Vec<String>>,
}
