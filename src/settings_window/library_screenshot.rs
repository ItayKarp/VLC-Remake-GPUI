//! A screenshot test of the Library page against `settings/screens/library/library.png`,
//! beside the other pages' tests in `screenshots.rs`.

use gpui::AppContext as _;

use super::SettingsWindow;
use crate::screenshot_test::{self, Shot};
use crate::settings::registry::Category;
use crate::window_chrome::TITLE_BAR_HEIGHT;

/// The window inside the design PNG's drop shadow, in the PNG's pixels and in logical pixels.
const DESIGN_ORIGIN: (u32, u32) = (30, 30);
const DESIGN_SIZE: (f32, f32) = (1048., 860.);
/// The share of differing pixels: the text, the Media Folders list the design doesn't have
/// (S5.9.3), which moves the other sections down, Only allow one VLC window, on by default where
/// the design shows it off (and its dependents undimmed), and Apply to files opened from the file
/// manager, which macOS doesn't show, and Fetch titles and artwork's default. Measured near 3.7%
/// on macOS (November 2026).
const MAX_MISMATCH: f64 = 0.05;

#[test]
fn the_library_page_matches_the_design() {
    // macOS gets a strip for the traffic lights, which the design leaves out.
    let strip = if cfg!(target_os = "macos") { TITLE_BAR_HEIGHT } else { 0. };
    let shot = Shot {
        name: "settings-library",
        design: "docs/ui/settings/screens/library/library.png".into(),
        design_origin: DESIGN_ORIGIN,
        window: (DESIGN_SIZE.0, DESIGN_SIZE.1 + strip),
        skip_top: strip,
        max_mismatch: MAX_MISMATCH,
    };
    let failure = screenshot_test::check(&shot, |window, cx| {
        super::bind_keys(cx);
        cx.new(|cx| SettingsWindow::new(Category::Library, window, cx))
    });
    assert!(failure.is_none(), "{}", failure.unwrap_or_default());
}
