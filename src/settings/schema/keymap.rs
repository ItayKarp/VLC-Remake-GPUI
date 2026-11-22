//! Custom key bindings (S5.6.3): the shortcuts the user changed in the Keyboard Shortcuts
//! dialog. Commands not listed keep their default keys.
//!
//! The changes are a list of tables rather than a table keyed by command, because saving merges
//! tables key by key: a command reset to its defaults would survive in the file. A list is
//! replaced whole.

use serde::{Deserialize, Serialize};

use super::super::lenient::lenient;

/// Custom key bindings.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeymapSettings {
    /// One entry per command whose keys the user changed, in no particular order.
    #[serde(deserialize_with = "lenient")]
    pub bindings: Vec<CustomBinding>,
}

/// One command's keys, replacing its defaults.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomBinding {
    /// The command's action name (`vlc::Stop`).
    pub command: String,
    /// Its keys in GPUI's syntax (`secondary-o`, `shift-left`), the primary one first; empty
    /// when the user cleared them.
    pub keys: Vec<String>,
}

impl CustomBinding {
    /// `command`'s keys set to `keys`.
    pub fn new(command: impl Into<String>, keys: Vec<String>) -> Self {
        Self { command: command.into(), keys }
    }
}
