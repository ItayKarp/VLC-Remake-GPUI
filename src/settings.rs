//! Platform paths and the (tiny) persisted settings written by onboarding.

use std::fs;
use std::path::{Path, PathBuf};

use crate::theme::{Accent, ThemeMode};

pub const APP_DIR_NAME: &str = "RedesignedVLC";

fn home_dir() -> Option<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var).map(PathBuf::from)
}

/// The OS's conventional video folder (`~/Movies` on macOS, `~/Videos` elsewhere).
pub fn default_media_folder() -> PathBuf {
    let home = home_dir().unwrap_or_default();
    if cfg!(target_os = "macos") {
        home.join("Movies")
    } else {
        home.join("Videos")
    }
}

/// Shortens the home directory prefix to `~` for display, like the design's `~/Videos`.
pub fn display_path(path: &Path) -> String {
    if let Some(home) = home_dir()
        && let Ok(rest) = path.strip_prefix(&home)
    {
        if rest.as_os_str().is_empty() {
            return "~".into();
        }
        return format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display());
    }
    path.display().to_string()
}

/// Per-user config directory for the app.
pub fn config_dir() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        home_dir().map(|home| home.join("Library").join("Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| home_dir().map(|home| home.join(".config")))
    }?;
    Some(base.join(APP_DIR_NAME))
}

/// Choices made during onboarding.
pub struct OnboardingResult {
    pub media_folder: PathBuf,
    pub theme: ThemeMode,
    pub accent: Accent,
}

impl OnboardingResult {
    pub fn save(&self) -> std::io::Result<PathBuf> {
        let dir = config_dir().ok_or_else(|| std::io::Error::other("no config directory"))?;
        fs::create_dir_all(&dir)?;
        let path = dir.join("settings.toml");
        let folder = self.media_folder.display().to_string().replace('\\', "\\\\");
        let contents = format!(
            "onboarding_complete = true\nmedia_folder = \"{folder}\"\ntheme = \"{}\"\naccent = \"{}\"\n",
            self.theme.key(),
            self.accent.key(),
        );
        fs::write(&path, contents)?;
        Ok(path)
    }
}
