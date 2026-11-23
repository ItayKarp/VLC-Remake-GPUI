//! Thumbnail tiles (S4.4.3): a media's picture at a library size, or its placeholder while the
//! thumbnailer works on it (or when it has none). The placeholders follow the designs: a grey
//! tile for video, a green music-note tile for a song without a cover (`sidebar-home`), and the
//! playlist glyph for a playlist (`media_cards/playlist-*`).

use gpui::{App, Div, Hsla, Window, div, img, prelude::*, px, rgb};

use super::icon::icon;
use crate::assets::icons;
use crate::db::thumbnails::SizeClass;
use crate::library::thumbnail_cache;
use crate::library::thumbnailer::Thumbnail;
use crate::theme::{self, Palette};

/// The video placeholder's grey, from the PRD (`#303030`), and its light-theme counterpart.
const VIDEO_DARK: u32 = 0x303030;
const VIDEO_LIGHT: u32 = 0xD9D9D9;
/// The song placeholder's tile and note, sampled from `sidebar-home`, and light counterparts.
const SONG_TILE_DARK: u32 = 0x303D34;
const SONG_NOTE_DARK: u32 = 0x1ED760;
const SONG_TILE_LIGHT: u32 = 0xD3E6D8;
const SONG_NOTE_LIGHT: u32 = 0x138A3D;
/// The playlist glyph's share of the tile's shorter side.
const GLYPH_SHARE: f32 = 0.625;
/// The music note's icon box fills the tile; the note itself covers about two thirds of it.
const NOTE_SHARE: f32 = 0.95;
/// The tiles' corner radius, from the designs.
const TILE_RADIUS: f32 = 2.;
/// Above this display scale, the 2× pictures are drawn.
const HIDPI_ABOVE: f32 = 1.;

/// What stands in for a missing picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placeholder {
    /// A video: a plain grey tile.
    Video,
    /// A song without a cover: a green tile with a music note.
    Song,
    /// A playlist: its glyph.
    #[allow(dead_code, reason = "the playlist views (S4.6, `sidebar-playlists`) draw it")]
    Playlist,
}

/// A thumbnail tile of one size class.
#[derive(IntoElement)]
pub struct ThumbnailTile {
    class: SizeClass,
    placeholder: Placeholder,
    thumbnail: Option<Thumbnail>,
    /// Drawn this size instead of the class's, in logical pixels.
    size: Option<(f32, f32)>,
    /// The corners' radius, in logical pixels.
    radius: f32,
    /// Draws the dark theme's placeholder whatever the theme.
    always_dark: bool,
}

impl ThumbnailTile {
    /// A `class` tile showing `placeholder` until a picture is given.
    pub fn new(class: SizeClass, placeholder: Placeholder) -> Self {
        Self {
            class,
            placeholder,
            thumbnail: None,
            size: None,
            radius: TILE_RADIUS,
            always_dark: false,
        }
    }

    /// Draws the tile `width` by `height` instead of its class's size; the class still picks
    /// the picture, which is scaled to fit.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = Some((width, height));
        self
    }

    /// Rounds the corners by `radius` logical pixels instead of the library's small radius
    /// (music mode's large cover, S8.1.3).
    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    /// Draws the dark theme's placeholder in both themes, for a tile on an area that is dark
    /// whatever the theme (music mode's cover, on the black video area, S8.1.3).
    pub fn always_dark(mut self) -> Self {
        self.always_dark = true;
        self
    }

    /// Shows `thumbnail`'s picture, if there is one.
    pub fn picture(mut self, thumbnail: Option<&Thumbnail>) -> Self {
        self.thumbnail = thumbnail.cloned();
        self
    }
}

impl RenderOnce for ThumbnailTile {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (width, height) = self.size.unwrap_or_else(|| logical_size(self.class));
        let tile =
            div().flex_none().w(px(width)).h(px(height)).rounded(px(self.radius)).overflow_hidden();
        let scale = scale_for(window.scale_factor());
        let path = self.thumbnail.as_ref().and_then(|t| t.path(self.class, scale));
        if let Some(path) = path {
            return tile.child(img(path.to_path_buf()).size_full());
        }
        let palette = theme::palette(window, cx);
        let dark = self.always_dark || palette.dark;
        placeholder(tile, self.placeholder, width.min(height), dark, &palette)
    }
}

/// `tile` dressed as `placeholder` in the `dark` or light theme's colours, with its glyph sized
/// for a `side`-pixel tile.
fn placeholder(
    tile: Div,
    placeholder: Placeholder,
    side: f32,
    dark: bool,
    palette: &Palette,
) -> Div {
    // Centering does nothing to the glyph-less video tile.
    let tile = tile.flex().items_center().justify_center();
    let pick = |dark_color: u32, light_color: u32| -> Hsla {
        rgb(if dark { dark_color } else { light_color }).into()
    };
    match placeholder {
        Placeholder::Video => tile.bg(pick(VIDEO_DARK, VIDEO_LIGHT)),
        Placeholder::Song => tile.bg(pick(SONG_TILE_DARK, SONG_TILE_LIGHT)).child(icon(
            icons::MUSIC_NOTE,
            side * NOTE_SHARE,
            pick(SONG_NOTE_DARK, SONG_NOTE_LIGHT),
        )),
        Placeholder::Playlist => {
            tile.child(icon(icons::PLAYLIST, side * GLYPH_SHARE, palette.icon))
        }
    }
}

/// A `class` tile's size in logical pixels.
pub fn logical_size(class: SizeClass) -> (f32, f32) {
    let (width, height) = match class {
        SizeClass::List => thumbnail_cache::LIST_SIZE,
        SizeClass::Grid => thumbnail_cache::GRID_SIZE,
    };
    (width as f32, height as f32)
}

/// Which pictures a display at `scale_factor` draws: 1, or 2 for HiDPI.
pub fn scale_for(scale_factor: f32) -> u8 {
    if scale_factor > HIDPI_ABOVE { 2 } else { 1 }
}

#[cfg(test)]
mod screenshots;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidpi_displays_draw_the_2x_pictures() {
        assert_eq!(scale_for(1.), 1);
        assert_eq!(scale_for(1.5), 2);
        assert_eq!(scale_for(2.), 2);
    }

    #[test]
    fn tiles_have_the_cached_sizes() {
        assert_eq!(logical_size(SizeClass::List), (32., 32.));
        assert_eq!(logical_size(SizeClass::Grid), (320., 180.));
    }
}
