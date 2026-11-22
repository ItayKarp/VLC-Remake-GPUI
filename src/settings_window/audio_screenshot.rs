//! Screenshot tests of the Audio page against `settings/screens/audio/audio_01.png` and, for the
//! Effects section and its equalizer, `audio_02.png`, beside the other pages' tests in
//! `screenshots.rs`.

use gpui::AppContext as _;

use super::SettingsWindow;
use crate::screenshot_test::{self, Shot};
use crate::settings::registry::Category;
use crate::window_chrome::TITLE_BAR_HEIGHT;

/// The window inside the design PNG's drop shadow, in the PNG's pixels and in logical pixels.
const DESIGN_ORIGIN: (u32, u32) = (30, 30);
const DESIGN_SIZE: (f32, f32) = (1048., 860.);
/// The share of differing pixels: the text, the Pause when headphones are unplugged row the
/// design doesn't draw, which moves Volume down, and Startup volume drawn as a dropdown over its
/// own Always starts at row. Measured near 3.6% on macOS (November 2026).
const MAX_MISMATCH: f64 = 0.05;

#[test]
fn the_audio_page_matches_the_design() {
    // macOS gets a strip for the traffic lights, which the design leaves out.
    let strip = if cfg!(target_os = "macos") { TITLE_BAR_HEIGHT } else { 0. };
    let shot = Shot {
        name: "settings-audio",
        design: "docs/ui/settings/screens/audio/audio_01.png".into(),
        design_origin: DESIGN_ORIGIN,
        window: (DESIGN_SIZE.0, DESIGN_SIZE.1 + strip),
        skip_top: strip,
        max_mismatch: MAX_MISMATCH,
    };
    let failure = screenshot_test::check(&shot, |window, cx| {
        super::bind_keys(cx);
        cx.new(|cx| SettingsWindow::new(Category::Audio, window, cx))
    });
    assert!(failure.is_none(), "{}", failure.unwrap_or_default());
}

/// The share of differing pixels in the Effects view: the text, the search's marks, and the
/// spatial audio rows the search leaves out. Measured near 3.6% on macOS (November 2026).
const EFFECTS_MAX_MISMATCH: f64 = 0.05;

#[test]
fn the_audio_effects_match_the_design() {
    let strip = if cfg!(target_os = "macos") { TITLE_BAR_HEIGHT } else { 0. };
    let shot = Shot {
        name: "settings-audio-effects",
        design: "docs/ui/settings/screens/audio/audio_02.png".into(),
        design_origin: DESIGN_ORIGIN,
        window: (DESIGN_SIZE.0, DESIGN_SIZE.1 + strip),
        skip_top: strip,
        max_mismatch: EFFECTS_MAX_MISMATCH,
    };
    // A search brings the Effects section, which the page shows below the fold, to the top.
    let failure = screenshot_test::check(&shot, |window, cx| {
        super::bind_keys(cx);
        let view = cx.new(|cx| SettingsWindow::new(Category::Audio, window, cx));
        view.update(cx, |view, cx| {
            view.query.update(cx, |query, cx| query.set_text("equalizer", cx));
            view.search_changed(window, cx);
        });
        view
    });
    assert!(failure.is_none(), "{}", failure.unwrap_or_default());
}
