//! Assets compiled into the binary, so the app works without any files next to
//! the executable (important for both the Windows .exe and the macOS .app).
//!
//! # Icon pipeline (S0.4.13)
//!
//! Icons come from the design library in `docs/icons/**` (24×24, `#8F8F8F`
//! strokes, accent parts in `#AB6314`/`#D37918`). Copy the ones a stage needs into
//! `assets/icons/` under a short name, then add them to [`icons!`] below; that
//! embeds the file and defines the constant in one place.
//!
//! GPUI's `svg()` paints a single-color mask, so the designs' colors are ignored
//! and the caller picks one (`text_color`). The `two_tone=yes` icons are split
//! into two layered files instead of being rasterized, so they stay sharp at any
//! scale and follow the accent at runtime:
//!
//! - `<name>.svg` holds the base (gray) paths,
//! - `<name>.accent.svg` holds the accent paths, with the same `viewBox`.
//!
//! Declare them with [`TwoTone`] and draw them with `ui::icon::two_tone`.
//! Tests check that every constant resolves and that both layers line up.

use std::borrow::Cow;

use anyhow::Result;
use gpui::{AssetSource, SharedString};

/// Declares the embedded icons: each `CONST => "file.svg"` becomes an
/// `icons::CONST` path and an entry in the asset table.
macro_rules! icons {
    ($($(#[$meta:meta])* $name:ident => $file:literal),* $(,)?) => {
        /// Icon paths, so call sites can't typo an asset name.
        pub mod icons {
            $(
                $(#[$meta])*
                pub const $name: &str = concat!("icons/", $file);
            )*

            /// Every icon constant, for the resolution test.
            #[cfg(test)]
            pub const ALL: &[&str] = &[$($name),*];
        }

        static ASSETS: &[(&str, &[u8])] = &[$(
            (
                concat!("icons/", $file),
                include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/", $file))
                    as &[u8],
            ),
        )*];
    };
}

icons! {
    LOGO => "vlc-logo.svg",
    LOGO_SMALL => "vlc-small.svg",
    PLAY => "play.svg",
    PAUSE => "pause.svg",
    /// The Play/Pause button while opening or buffering (S2.2.2; not in the designs).
    SPINNER => "spinner.svg",
    PREVIOUS => "previous.svg",
    NEXT => "next.svg",
    MINIMIZE => "minimize.svg",
    MAXIMIZE => "maximize.svg",
    FULLSCREEN => "fullscreen.svg",
    CLOSE => "close.svg",
    /// The speaker at a high level; the lower levels and mute follow (S2.5.4).
    VOLUME => "volume.svg",
    VOLUME_MEDIUM => "volume-medium.svg",
    VOLUME_LOW => "volume-low.svg",
    VOLUME_NONE => "volume-none.svg",
    /// Muted (not in the designs): the speaker struck through.
    VOLUME_MUTED => "volume-muted.svg",
    AUDIO => "audio.svg",
    SUBTITLES => "subtitles.svg",
    SETTINGS => "settings.svg",
    LIST => "list.svg",
    // The track popover (S2.6).
    VIDEO => "video.svg",
    REFRESH => "refresh.svg",
    EDIT => "edit.svg",
    ADD => "add.svg",
    ARROW_LEFT => "arrow-left.svg",
    ARROW_RIGHT => "arrow-right.svg",
    ARROW_UP => "arrow-up.svg",
    ARROW_DOWN => "arrow-down.svg",
    THEME_SYSTEM => "theme-system.svg",
    MORE => "more.svg",
    /// The sidebar toggle arrives with the library (Stage 4).
    #[allow(dead_code, reason = "the sidebar toggle arrives with the library (Stage 4)")]
    SIDEBAR => "sidebar.svg",
    FOLDER => "folder.svg",
    FILE => "file.svg",
    RESTORE => "restore.svg",
    // Component kit (S0.4).
    CHEVRON_DOWN => "chevron-down.svg",
    CHEVRON_UP => "chevron-up.svg",
    CHEVRON_RIGHT => "chevron-right.svg",
    CHECK => "check.svg",
    INFO => "info.svg",
    INFO_TWO_TONE => "info-two-tone.svg",
    INFO_TWO_TONE_ACCENT => "info-two-tone.accent.svg",
    CHECK_BOX => "check-box.svg",
    CHECK_BOX_ACCENT => "check-box.accent.svg",
    // The main menu (S0.5.4).
    STREAMING => "streaming.svg",
    DISC => "disc.svg",
    CAPTURE_DEVICE => "capture-device.svg",
    CONVERT => "convert.svg",
    RECENT => "recent.svg",
    #[allow(dead_code, reason = "the main menu's Profile submenu arrives with profiles (S8.8)")]
    PROFILE => "profile.svg",
    HOTKEYS => "hotkeys.svg",
    PAINT => "paint.svg",
    // The video's right-click menu (S2.9.2).
    STOP => "stop.svg",
    PLAYBACK => "playback.svg",
    /// Hide UI (not in the icon set): the video's frame struck through, as the menu design
    /// draws it.
    HIDE_UI => "hide-ui.svg",
    QUIT => "quit.svg",
    // The Keyboard Shortcuts dialog (S2.10.4).
    SEARCH => "search.svg",
    // The List panel (S3.2).
    QUEUE => "queue.svg",
    BOOKMARKS => "bookmarks.svg",
    CHAPTERS => "chapters.svg",
    SORT => "sort.svg",
    REMOVE => "remove.svg",
    // Repeat and Shuffle (S3.3).
    REPEAT => "repeat.svg",
    /// Repeat One (not in the icon set): the repeat arrows around a 1.
    REPEAT_ONE => "repeat-one.svg",
    SHUFFLE => "shuffle.svg",
    /// A queue row's drag handle (`move` in the icon set).
    DRAG_HANDLE => "drag-handle.svg",
    // Saved playlists (S3.5).
    PLAYLIST => "playlist.svg",
}

/// A two-tone icon: a base layer in the icon color and an accent layer on top.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TwoTone {
    /// The layer drawn in the icon color.
    pub base: &'static str,
    /// The layer drawn in the accent color.
    pub accent: &'static str,
}

/// The two-tone icons, pairing each base layer with its accent layer.
pub mod two_tone {
    use super::{TwoTone, icons};

    /// The info icon, for toasts and banners.
    pub const INFO: TwoTone =
        TwoTone { base: icons::INFO_TWO_TONE, accent: icons::INFO_TWO_TONE_ACCENT };
    /// The check box icon.
    pub const CHECK_BOX: TwoTone =
        TwoTone { base: icons::CHECK_BOX, accent: icons::CHECK_BOX_ACCENT };

    #[cfg(test)]
    pub const ALL: &[TwoTone] = &[INFO, CHECK_BOX];
}

/// Serves the embedded assets to GPUI by path.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(ASSETS.iter().find(|(name, _)| *name == path).map(|(_, data)| Cow::Borrowed(*data)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(ASSETS
            .iter()
            .filter(|(name, _)| name.starts_with(path))
            .map(|(name, _)| SharedString::from(*name))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(path: &str) -> String {
        let bytes = Assets
            .load(path)
            .unwrap()
            .unwrap_or_else(|| panic!("{path} doesn't resolve to an embedded asset"));
        String::from_utf8(bytes.into_owned()).unwrap()
    }

    fn view_box(svg: &str) -> &str {
        let start = svg.find("viewBox=\"").expect("svg without a viewBox") + 9;
        let len = svg[start..].find('"').unwrap();
        &svg[start..start + len]
    }

    #[test]
    fn every_icon_constant_resolves() {
        for path in icons::ALL {
            let svg = load(path);
            assert!(svg.contains("<svg"), "{path} isn't an svg");
        }
    }

    #[test]
    fn icon_paths_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for path in icons::ALL {
            assert!(seen.insert(path), "{path} is declared twice");
        }
    }

    #[test]
    fn two_tone_layers_line_up() {
        for icon in two_tone::ALL {
            let base = load(icon.base);
            let accent = load(icon.accent);
            assert_eq!(
                view_box(&base),
                view_box(&accent),
                "{} and {} have different viewBoxes",
                icon.base,
                icon.accent
            );
            assert!(
                icon.accent.ends_with(".accent.svg"),
                "{} should be named <base>.accent.svg",
                icon.accent
            );
        }
    }

    #[test]
    fn list_finds_icons() {
        let listed = Assets.list("icons/").unwrap();
        assert_eq!(listed.len(), icons::ALL.len());
    }
}
