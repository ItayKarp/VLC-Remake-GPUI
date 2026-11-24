//! A screenshot test of the Video page against `settings/screens/video/video_01.png`, beside the
//! other pages' tests in `screenshots.rs`.

use gpui::AppContext as _;

use super::SettingsWindow;
use crate::screenshot_test::{self, Shot};
use crate::settings::registry::Category;
use crate::window_chrome::TITLE_BAR_HEIGHT;

/// The window inside the design PNG's drop shadow, in the PNG's pixels and in logical pixels.
const DESIGN_ORIGIN: (u32, u32) = (30, 30);
const DESIGN_SIZE: (f32, f32) = (1048., 860.);
/// The share of differing pixels: the text, the (i) icons the Picture rows have and the design
/// doesn't, and the Play video as desktop wallpaper row the page leaves out except on Windows,
/// which moves every row under it up. Measured near 4.2% on macOS (November 2026).
const MAX_MISMATCH: f64 = 0.06;

#[test]
fn the_video_page_matches_the_design() {
    // macOS gets a strip for the traffic lights, which the design leaves out.
    let strip = if cfg!(target_os = "macos") { TITLE_BAR_HEIGHT } else { 0. };
    let shot = Shot {
        name: "settings-video",
        design: "docs/ui/settings/screens/video/video_01.png".into(),
        design_origin: DESIGN_ORIGIN,
        window: (DESIGN_SIZE.0, DESIGN_SIZE.1 + strip),
        skip_top: strip,
        max_mismatch: MAX_MISMATCH,
    };
    let failure = screenshot_test::check(&shot, |window, cx| {
        super::bind_keys(cx);
        cx.new(|cx| SettingsWindow::new(Category::Video, window, cx))
    });
    assert!(failure.is_none(), "{}", failure.unwrap_or_default());
}
