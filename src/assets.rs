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

use crate::engine::audio::equalizer::Preset;
use crate::engine::audio::visualizer::Visualizer;

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
    /// Music mode's Back 15 s and Forward 15 s (S8.1.2, `controls-music.png`).
    BACK_15 => "back-15.svg",
    FORWARD_15 => "forward-15.svg",
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
    /// A subtitle file in the track popover's External Tracks (S6.1.5; `subtitle` in the
    /// settings categories' icons).
    SUBTITLE_FILE => "subtitle-file.svg",
    /// Switching to another subtitle file (S6.1.5).
    SWAP => "swap.svg",
    SETTINGS => "settings.svg",
    LIST => "list.svg",
    // The track popover (S2.6).
    VIDEO => "video.svg",
    REFRESH => "refresh.svg",
    REPLAY => "replay.svg",
    EDIT => "edit.svg",
    ADD => "add.svg",
    ARROW_LEFT => "arrow-left.svg",
    ARROW_RIGHT => "arrow-right.svg",
    ARROW_UP => "arrow-up.svg",
    ARROW_DOWN => "arrow-down.svg",
    THEME_SYSTEM => "theme-system.svg",
    MORE => "more.svg",
    /// The title bar's library sidebar toggle.
    SIDEBAR => "sidebar.svg",
    FOLDER => "folder.svg",
    FILE => "file.svg",
    RESTORE => "restore.svg",
    /// The crop panel's linked edges (S7.2.5): joined, and apart.
    LINK => "link.svg",
    LINK_OFF => "link-off.svg",
    // Component kit (S0.4).
    CHEVRON_DOWN => "chevron-down.svg",
    CHEVRON_UP => "chevron-up.svg",
    CHEVRON_RIGHT => "chevron-right.svg",
    CHEVRON_LEFT => "chevron-left.svg",
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
    /// A song without a cover (S4.4.3; not in the icon set): two beamed notes, as the designs'
    /// placeholder tile draws them.
    MUSIC_NOTE => "music-note.svg",
    // The library's rail and header (S4.5). Each section has an outline icon and a filled one;
    // the filled one is orange at the section's root and grey deeper in it.
    HOME => "home.svg",
    HOME_FILLED => "home-filled.svg",
    VIDEOS => "videos.svg",
    VIDEOS_FILLED => "videos-filled.svg",
    SONGS => "songs.svg",
    SONGS_FILLED => "songs-filled.svg",
    STREAMING_FILLED => "streaming-filled.svg",
    /// Playlists' filled rail icon: the playlist glyph with heavier strokes (the set's "active").
    PLAYLIST_FILLED => "playlist-filled.svg",
    POP_OUT => "pop-out.svg",
    /// The View button and menu (`view_options`): a grid of tiles, and a list of rows.
    GRID_VIEW => "grid-view.svg",
    LIST_VIEW => "list-view.svg",
    // Music's categories (`sidebar-music`).
    MUSIC => "music.svg",
    ALBUM => "album.svg",
    ARTIST => "artist.svg",
    GENRE => "genre.svg",
    // The library's file, folder and playlist menus (S4.8).
    FAVORITE => "favorite.svg",
    /// Mark as Watched: [`two_tone::CHECK_BOX`]'s two layers in one file, as menus draw one color.
    CHECK_BOX_FULL => "check-box-full.svg",
    NEW_FOLDER => "new-folder.svg",
    SAVE => "save.svg",
    // The Settings window's sidebar (S5.2.2); the other categories reuse icons above.
    /// General and Performance: three sliders (`preferences` in the icon set).
    PREFERENCES => "preferences.svg",
    /// Interface: a pointer.
    INTERFACE => "interface.svg",
    /// Privacy: a shield.
    PRIVACY => "privacy.svg",
    /// Recording: a record button.
    RECORD => "record.svg",
    /// Take Snapshot (S8.4, `media_controls/snapshot`).
    SNAPSHOT => "snapshot.svg",
    /// Extensions: a puzzle piece (`plugins` in the icon set).
    PLUGINS => "plugins.svg",
    // The equalizer presets' curves (S7.7.3, `docs/icons/eq`): one line under each, and each
    // preset's shape over it, drawn in the accent. Flat is the line alone.
    EQ_LINE => "eq-line.svg",
    EQ_FLAT => "eq-flat.accent.svg",
    EQ_CLASSICAL => "eq-classical.accent.svg",
    EQ_CLUB => "eq-club.accent.svg",
    EQ_DANCE => "eq-dance.accent.svg",
    EQ_FULL_BASS => "eq-full-bass.accent.svg",
    EQ_FULL_BASS_AND_TREBLE => "eq-full-bass-and-treble.accent.svg",
    EQ_FULL_TREBLE => "eq-full-treble.accent.svg",
    EQ_HEADPHONES => "eq-headphones.accent.svg",
    EQ_LARGE_HALL => "eq-large-hall.accent.svg",
    EQ_LIVE => "eq-live.accent.svg",
    EQ_PARTY => "eq-party.accent.svg",
    EQ_POP => "eq-pop.accent.svg",
    EQ_REGGAE => "eq-reggae.accent.svg",
    EQ_ROCK => "eq-rock.accent.svg",
    EQ_SKA => "eq-ska.accent.svg",
    EQ_SOFT => "eq-soft.accent.svg",
    EQ_SOFT_ROCK => "eq-soft-rock.accent.svg",
    EQ_TECHNO => "eq-techno.accent.svg",
    // The visualizers (S7.8.1, `docs/icons/audio_visualizer`), each split in two layers.
    VIZ_SPECTROMETER => "viz-spectrometer.svg",
    VIZ_SPECTROMETER_ACCENT => "viz-spectrometer.accent.svg",
    VIZ_SCOPE => "viz-scope.svg",
    VIZ_SCOPE_ACCENT => "viz-scope.accent.svg",
    VIZ_SPECTRUM => "viz-spectrum.svg",
    VIZ_SPECTRUM_ACCENT => "viz-spectrum.accent.svg",
    VIZ_WAVEFORM => "viz-waveform.svg",
    VIZ_WAVEFORM_ACCENT => "viz-waveform.accent.svg",
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

    /// The visualizers' glyphs (S7.8.1).
    pub const SPECTROMETER: TwoTone =
        TwoTone { base: icons::VIZ_SPECTROMETER, accent: icons::VIZ_SPECTROMETER_ACCENT };
    /// Scope's glyph.
    pub const SCOPE: TwoTone = TwoTone { base: icons::VIZ_SCOPE, accent: icons::VIZ_SCOPE_ACCENT };
    /// Spectrum's glyph.
    pub const SPECTRUM: TwoTone =
        TwoTone { base: icons::VIZ_SPECTRUM, accent: icons::VIZ_SPECTRUM_ACCENT };
    /// Waveform's glyph.
    pub const WAVEFORM: TwoTone =
        TwoTone { base: icons::VIZ_WAVEFORM, accent: icons::VIZ_WAVEFORM_ACCENT };

    #[cfg(test)]
    pub const ALL: &[TwoTone] = &[INFO, CHECK_BOX, SPECTROMETER, SCOPE, SPECTRUM, WAVEFORM];
}

/// The glyph before a visualizer's name in its dropdown; Disabled has none.
pub fn visualizer_glyph(visualizer: Visualizer) -> Option<TwoTone> {
    match visualizer {
        Visualizer::Disabled => None,
        Visualizer::Spectrometer => Some(two_tone::SPECTROMETER),
        Visualizer::Scope => Some(two_tone::SCOPE),
        Visualizer::Spectrum => Some(two_tone::SPECTRUM),
        Visualizer::Waveform => Some(two_tone::WAVEFORM),
    }
}

/// The curve before an equalizer preset's name in the presets dropdown; Custom has none.
pub fn equalizer_glyph(preset: Preset) -> Option<TwoTone> {
    let accent = match preset {
        Preset::Custom => return None,
        Preset::Flat => icons::EQ_FLAT,
        Preset::Classical => icons::EQ_CLASSICAL,
        Preset::Club => icons::EQ_CLUB,
        Preset::Dance => icons::EQ_DANCE,
        Preset::FullBass => icons::EQ_FULL_BASS,
        Preset::FullBassAndTreble => icons::EQ_FULL_BASS_AND_TREBLE,
        Preset::FullTreble => icons::EQ_FULL_TREBLE,
        Preset::Headphones => icons::EQ_HEADPHONES,
        Preset::LargeHall => icons::EQ_LARGE_HALL,
        Preset::Live => icons::EQ_LIVE,
        Preset::Party => icons::EQ_PARTY,
        Preset::Pop => icons::EQ_POP,
        Preset::Reggae => icons::EQ_REGGAE,
        Preset::Rock => icons::EQ_ROCK,
        Preset::Ska => icons::EQ_SKA,
        Preset::Soft => icons::EQ_SOFT,
        Preset::SoftRock => icons::EQ_SOFT_ROCK,
        Preset::Techno => icons::EQ_TECHNO,
    };
    Some(TwoTone { base: icons::EQ_LINE, accent })
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
