//! What the updater remembers between runs (S9.5), in `updates.toml` next to the machine-local
//! settings: when it last checked, the version the user skipped, and the update on trial, which
//! is rolled back if it fails to start twice (S9.5.2).

use std::path::{Path, PathBuf};

use semver::Version;
use serde::{Deserialize, Serialize};

use crate::settings::Locations;

/// The state file's name, next to the machine-local settings.
const FILE_NAME: &str = "updates.toml";

/// The updater's memory.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateState {
    /// When the last check finished, in seconds since the Unix epoch.
    pub last_check_s: Option<i64>,
    /// The version the user skipped (or that was rolled back): it and older ones aren't
    /// offered.
    pub skipped: Option<Version>,
    /// The update installed at the last quit, until it has started once.
    pub trial: Option<Trial>,
    /// A rolled-back update, to tell the user about once.
    pub rolled_back: Option<RolledBack>,
}

/// An installed update that hasn't proved it starts yet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trial {
    /// The version installed.
    pub version: Version,
    /// The version it replaced.
    pub previous: Version,
    /// How many times it was launched without starting.
    pub starts: u32,
    /// How to put the previous version back.
    pub backup: Backup,
}

/// Where the version an update replaced is kept, until the update has started.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Backup {
    /// The app bundle or AppImage at `installed` was moved to `saved`.
    Moved {
        /// Where the app is installed.
        installed: PathBuf,
        /// Where the previous version is.
        saved: PathBuf,
    },
    /// Windows Installer packages: the one installed, and the previous version's if the
    /// updater installed that one too (otherwise there is nothing to go back to).
    Msi {
        /// The package installed.
        installed: PathBuf,
        /// The previous version's package.
        previous: Option<PathBuf>,
    },
}

/// An update that failed to start and was replaced by the version before it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolledBack {
    /// The update that failed.
    pub version: Version,
}

impl UpdateState {
    /// The state saved in `dir`, or a fresh one when there is none or it can't be read.
    pub fn load(dir: &Path) -> Self {
        let text = std::fs::read_to_string(dir.join(FILE_NAME)).unwrap_or_default();
        toml::from_str(&text).unwrap_or_else(|error| {
            tracing::warn!("updates: {FILE_NAME} can't be read, starting afresh: {error}");
            Self::default()
        })
    }

    /// Saves the state in `dir`, logging a failure (the updater works on without it).
    pub fn save(&self, dir: &Path) {
        let written = toml::to_string(self).map_err(std::io::Error::other).and_then(|text| {
            std::fs::create_dir_all(dir)?;
            std::fs::write(dir.join(FILE_NAME), text)
        });
        if let Err(error) = written {
            tracing::warn!("updates: couldn't save {FILE_NAME}: {error}");
        }
    }
}

/// The folder the state, kept installers and downloads live in: next to the machine-local
/// settings. `None` when settings live in memory only; the updater then does nothing.
pub fn data_dir() -> Option<PathBuf> {
    let locations = Locations::resolve()?;
    Some(locations.local.parent()?.join("updates"))
}

/// Seconds since the Unix epoch.
pub fn now_s() -> i64 {
    crate::db::now_ms() / 1000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_its_file() {
        let dir = std::env::temp_dir().join(format!("rvlc-updates-state-{}", std::process::id()));
        let state = UpdateState {
            last_check_s: Some(5),
            skipped: Some(Version::new(0, 3, 0)),
            trial: Some(Trial {
                version: Version::new(0, 2, 0),
                previous: Version::new(0, 1, 0),
                starts: 1,
                backup: Backup::Moved {
                    installed: "/Applications/VLC.app".into(),
                    saved: "/Applications/.VLC-previous.app".into(),
                },
            }),
            rolled_back: None,
        };
        state.save(&dir);
        assert_eq!(UpdateState::load(&dir), state);
        std::fs::write(dir.join(FILE_NAME), "trial = 3").unwrap();
        assert_eq!(UpdateState::load(&dir), UpdateState::default());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_missing_file_is_a_fresh_state() {
        assert_eq!(UpdateState::load(Path::new("/nonexistent/rvlc")), UpdateState::default());
    }
}
