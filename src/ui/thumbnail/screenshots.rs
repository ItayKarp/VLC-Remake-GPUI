//! Screenshot tests of the placeholders against the designs: the song tile in `sidebar-home`
//! (Listen Again) and the video tile in `media_cards/media-unwatched`.

use gpui::{AppContext as _, Context, IntoElement, Render, Window, div, prelude::*, rgb};

use super::{Placeholder, ThumbnailTile};
use crate::db::thumbnails::SizeClass;
use crate::screenshot_test::{self, Shot};

/// What the designs draw around the tiles.
const SIDEBAR: u32 = 0x0E0E0E;
/// The share of pixels allowed to differ for the song tile: its note is redrawn (it isn't in the
/// icon set), and a one-pixel line a fraction of a pixel off differs along its whole length.
/// Measured near 15% on macOS (November 2026).
const SONG_MAX_MISMATCH: f64 = 0.18;
/// The same for the plain video tile: its rounded corners. Measured near 0.5%.
const VIDEO_MAX_MISMATCH: f64 = 0.02;

struct TileShot {
    placeholder: Placeholder,
    background: u32,
}

impl Render for TileShot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(self.background))
            .child(ThumbnailTile::new(SizeClass::List, self.placeholder))
    }
}

fn check(shot: &Shot, placeholder: Placeholder, background: u32) -> Option<String> {
    screenshot_test::check(shot, |_, cx| cx.new(|_| TileShot { placeholder, background }))
}

#[test]
fn the_placeholders_match_the_designs() {
    let song = Shot {
        name: "thumbnail-song",
        design: "docs/ui/library/sidebar/sidebar-home.png".into(),
        design_origin: (140, 815),
        window: (32., 32.),
        skip_top: 0.,
        max_mismatch: SONG_MAX_MISMATCH,
    };
    let video = Shot {
        name: "thumbnail-video",
        design: "docs/ui/library/media_cards/media-unwatched.png".into(),
        design_origin: (0, 0),
        window: (32., 32.),
        skip_top: 0.,
        max_mismatch: VIDEO_MAX_MISMATCH,
    };
    let failures: Vec<String> =
        [check(&song, Placeholder::Song, SIDEBAR), check(&video, Placeholder::Video, 0xFFFFFF)]
            .into_iter()
            .flatten()
            .collect();
    assert!(failures.is_empty(), "a placeholder differs from its design:\n{}", failures.join("\n"));
}
