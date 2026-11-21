//! Interface preferences (Settings → Interface; its rows come with S5.4): what the player's
//! title bar shows (S2.1.2) and what the control bar's duration readout shows (S2.4.3).

use serde::{Deserialize, Serialize};

use super::super::lenient::lenient;

/// Interface preferences.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InterfaceSettings {
    /// "Show the file name in the title bar": the title bar and the OS window title name the
    /// media while it's loaded; off, they always say "VLC Media Player".
    #[serde(deserialize_with = "lenient")]
    pub show_file_name_in_title_bar: bool,
    /// Whether the duration readout shows the length or the time left; clicking it switches.
    #[serde(deserialize_with = "lenient")]
    pub duration_readout: DurationReadout,
}

/// What the control bar's duration readout shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DurationReadout {
    /// The media's length (`00:25:40`).
    #[default]
    Total,
    /// The media time left (`-00:23:30`), whatever the playback rate.
    Remaining,
}

impl DurationReadout {
    /// The other one.
    pub fn toggled(self) -> Self {
        match self {
            Self::Total => Self::Remaining,
            Self::Remaining => Self::Total,
        }
    }
}

impl Default for InterfaceSettings {
    fn default() -> Self {
        Self { show_file_name_in_title_bar: true, duration_readout: DurationReadout::default() }
    }
}
