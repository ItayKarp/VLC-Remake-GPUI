//! Where settings live, and path helpers for the UI.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The directory our settings live in, inside the platform config folders.
pub const APP_DIR_NAME: &str = "RedesignedVLC";
/// The roaming preferences file.
pub const PREFS_FILE_NAME: &str = "settings.toml";
/// The machine-local state file.
pub const LOCAL_FILE_NAME: &str = "local.toml";
/// The folder for logs and crash reports, next to the machine-local state.
pub const LOGS_DIR_NAME: &str = "logs";
/// The SQLite database of playback history and the library (S1.7), next to the local state.
pub const DATABASE_FILE_NAME: &str = "library.db";
/// The last session's queue (S3.4), next to the local state.
pub const SESSION_FILE_NAME: &str = "session.toml";
/// Subtitle files downloaded to link to media (S6.4), next to the local settings.
pub const SUBTITLES_DIR_NAME: &str = "subtitles";
/// The cache folder's name inside an overriding settings directory.
pub const CACHE_DIR_NAME: &str = "cache";
/// Overrides both settings directories (tests, portable mode, debugging).
pub const CONFIG_DIR_ENV: &str = "RVLC_CONFIG_DIR";

/// The two settings files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Locations {
    /// Roaming preferences (Windows: `%APPDATA%`).
    pub prefs: PathBuf,
    /// Machine-local state (Windows: `%LOCALAPPDATA%`).
    pub local: PathBuf,
}

impl Locations {
    /// Both files in one directory.
    pub fn in_dir(dir: &Path) -> Self {
        Locations { prefs: dir.join(PREFS_FILE_NAME), local: dir.join(LOCAL_FILE_NAME) }
    }

    /// The folder for logs and crash reports: `logs/` next to [`Self::local`].
    pub fn logs(&self) -> PathBuf {
        let dir = self.local.parent().unwrap_or_else(|| Path::new(""));
        dir.join(LOGS_DIR_NAME)
    }

    /// The playback history and library database: `library.db` next to [`Self::local`].
    pub fn database(&self) -> PathBuf {
        let dir = self.local.parent().unwrap_or_else(|| Path::new(""));
        dir.join(DATABASE_FILE_NAME)
    }

    /// The last session's queue: `session.toml` next to [`Self::local`].
    pub fn session(&self) -> PathBuf {
        let dir = self.local.parent().unwrap_or_else(|| Path::new(""));
        dir.join(SESSION_FILE_NAME)
    }

    /// Subtitle files downloaded From Link (S6.4): `subtitles/` next to [`Self::local`]. Not a
    /// cache: media keep them linked.
    pub fn subtitles(&self) -> PathBuf {
        let dir = self.local.parent().unwrap_or_else(|| Path::new(""));
        dir.join(SUBTITLES_DIR_NAME)
    }

    /// Resolves from `RVLC_CONFIG_DIR` or the platform's known folders. `None` if
    /// neither is available (some services and sandboxes): settings then live in
    /// memory only.
    pub fn resolve() -> Option<Self> {
        Self::resolve_with(config_dir_override())
    }

    /// Like [`Self::resolve`], with the override directory passed in (for tests).
    pub fn resolve_with(override_dir: Option<OsString>) -> Option<Self> {
        if let Some(dir) = override_dir.filter(|dir| !dir.is_empty()) {
            let dir = PathBuf::from(dir);
            // A relative override is resolved once so a later chdir can't move it.
            let dir = std::path::absolute(&dir).unwrap_or(dir);
            return Some(Self::in_dir(&dir));
        }
        // `dirs` uses the Known Folder API on Windows and doesn't depend on
        // `APPDATA`/`USERPROFILE` being set.
        let config = dirs::config_dir()?.join(APP_DIR_NAME);
        let local = dirs::state_dir()
            .or_else(dirs::data_local_dir)
            .map_or_else(|| config.clone(), |dir| dir.join(APP_DIR_NAME));
        Some(Locations { prefs: config.join(PREFS_FILE_NAME), local: local.join(LOCAL_FILE_NAME) })
    }
}

/// The folder for caches anyone may delete (library artwork, S4.3; thumbnails, S4.4): next to the
/// settings when `RVLC_CONFIG_DIR` overrides them, otherwise in the platform's cache folder.
/// `None` when neither is available: nothing is cached then.
pub fn cache_dir() -> Option<PathBuf> {
    match config_dir_override().filter(|dir| !dir.is_empty()) {
        Some(dir) => {
            let dir = PathBuf::from(dir).join(CACHE_DIR_NAME);
            Some(std::path::absolute(&dir).unwrap_or(dir))
        }
        None => Some(dirs::cache_dir()?.join(APP_DIR_NAME)),
    }
}

/// The folder `RVLC_CONFIG_DIR` names. Tests that don't name one get a folder of the test
/// process's own, so no test reads or writes the user's real settings, logs, crash reports or
/// caches.
fn config_dir_override() -> Option<OsString> {
    let named = std::env::var_os(CONFIG_DIR_ENV).filter(|dir| !dir.is_empty());
    if cfg!(test) { named.or_else(|| Some(test_config_dir())) } else { named }
}

/// The settings folder of this test process, under the system's temporary folder.
fn test_config_dir() -> OsString {
    std::env::temp_dir().join(format!("rvlc-test-config-{}", std::process::id())).into_os_string()
}

/// The OS's video folder. Uses the Known Folder API on Windows, so OneDrive folder
/// redirection is respected. Falls back to `<home>/Videos` (`~/Movies` on macOS).
pub fn default_media_folder() -> PathBuf {
    if let Some(dir) = dirs::video_dir() {
        return dir;
    }
    let home = dirs::home_dir().unwrap_or_default();
    if cfg!(target_os = "macos") { home.join("Movies") } else { home.join("Videos") }
}

/// `text` as a path, a leading `~` standing for the home folder, as path rows are typed.
pub fn expand_home(text: &str) -> PathBuf {
    match (text.strip_prefix('~'), dirs::home_dir()) {
        (Some(rest), Some(home)) => home.join(rest.trim_start_matches(['/', '\\'])),
        _ => Path::new(text).to_path_buf(),
    }
}

/// Shortens the home directory prefix to `~` for display, like the design's `~/Videos`.
pub fn display_path(path: &Path) -> String {
    if let Some(home) = dirs::home_dir()
        && let Ok(rest) = path.strip_prefix(&home)
    {
        if rest.as_os_str().is_empty() {
            return "~".into();
        }
        return format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display());
    }
    path.display().to_string()
}
