//! Assets compiled into the binary, so the app works without any files next to
//! the executable (important for both the Windows .exe and the macOS .app).

use std::borrow::Cow;

use anyhow::Result;
use gpui::{AssetSource, SharedString};

macro_rules! embed {
    ($($name:literal),* $(,)?) => {
        &[$(
            (
                concat!("icons/", $name),
                include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/", $name)) as &[u8],
            ),
        )*]
    };
}

static ASSETS: &[(&str, &[u8])] = embed![
    "vlc-logo.svg",
    "vlc-small.svg",
    "play.svg",
    "previous.svg",
    "next.svg",
    "minimize.svg",
    "maximize.svg",
    "fullscreen.svg",
    "close.svg",
    "volume.svg",
    "audio.svg",
    "subtitles.svg",
    "settings.svg",
    "list.svg",
    "arrow-left.svg",
    "arrow-right.svg",
    "arrow-up.svg",
    "arrow-down.svg",
    "theme-system.svg",
    "more.svg",
    "sidebar.svg",
    "folder.svg",
    "file.svg",
    "restore.svg",
];

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(ASSETS
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, data)| Cow::Borrowed(*data)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(ASSETS
            .iter()
            .filter(|(name, _)| name.starts_with(path))
            .map(|(name, _)| SharedString::from(*name))
            .collect())
    }
}

/// Icon paths, so call sites can't typo an asset name.
pub mod icons {
    pub const LOGO: &str = "icons/vlc-logo.svg";
    pub const LOGO_SMALL: &str = "icons/vlc-small.svg";
    pub const PLAY: &str = "icons/play.svg";
    pub const PREVIOUS: &str = "icons/previous.svg";
    pub const NEXT: &str = "icons/next.svg";
    pub const MINIMIZE: &str = "icons/minimize.svg";
    pub const MAXIMIZE: &str = "icons/maximize.svg";
    pub const FULLSCREEN: &str = "icons/fullscreen.svg";
    pub const CLOSE: &str = "icons/close.svg";
    pub const VOLUME: &str = "icons/volume.svg";
    pub const AUDIO: &str = "icons/audio.svg";
    pub const SUBTITLES: &str = "icons/subtitles.svg";
    pub const SETTINGS: &str = "icons/settings.svg";
    pub const LIST: &str = "icons/list.svg";
    pub const ARROW_LEFT: &str = "icons/arrow-left.svg";
    pub const ARROW_RIGHT: &str = "icons/arrow-right.svg";
    pub const ARROW_UP: &str = "icons/arrow-up.svg";
    pub const ARROW_DOWN: &str = "icons/arrow-down.svg";
    pub const THEME_SYSTEM: &str = "icons/theme-system.svg";
    pub const MORE: &str = "icons/more.svg";
    /// The sidebar toggle arrives with the library (Stage 4).
    #[allow(dead_code)]
    pub const SIDEBAR: &str = "icons/sidebar.svg";
    pub const FOLDER: &str = "icons/folder.svg";
    pub const FILE: &str = "icons/file.svg";
    pub const RESTORE: &str = "icons/restore.svg";
}
