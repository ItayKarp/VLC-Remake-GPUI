//! Interface preferences (Settings → Interface; its rows come with S5.4): what the player's
//! title bar shows (S2.1.2).

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
}

impl Default for InterfaceSettings {
    fn default() -> Self {
        Self { show_file_name_in_title_bar: true }
    }
}
