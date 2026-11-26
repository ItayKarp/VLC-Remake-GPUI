//! Screenshot tests of the General, Interface, Privacy and Controls pages against
//! `settings/screens/global/global_01.png`, `settings/screens/interface/interface_01.png`,
//! `settings/screens/privacy/privacy.png` and `settings/screens/controls/controls.png`.

use gpui::AppContext as _;

use super::SettingsWindow;
use crate::screenshot_test::{self, Shot};
use crate::settings::registry::Category;
use crate::window_chrome::TITLE_BAR_HEIGHT;

/// The window inside the design PNG's drop shadow, in the PNG's pixels and in logical pixels.
const DESIGN_ORIGIN: (u32, u32) = (30, 30);
const DESIGN_SIZE: (f32, f32) = (1048., 860.);
/// The share of pixels allowed to differ: the text, as the design uses Inter, the theme order
/// (D6 puts Light first), and the Run Setup Again row the design doesn't draw, which moves
/// Diagnostics down. Measured near 4.6% on macOS (November 2026).
const MAX_MISMATCH: f64 = 0.06;

#[test]
fn the_general_page_matches_the_design() {
    // macOS gets a strip for the traffic lights, which the design leaves out.
    let strip = if cfg!(target_os = "macos") { TITLE_BAR_HEIGHT } else { 0. };
    let shot = Shot {
        name: "settings-general",
        design: "docs/ui/settings/screens/global/global_01.png".into(),
        design_origin: DESIGN_ORIGIN,
        window: (DESIGN_SIZE.0, DESIGN_SIZE.1 + strip),
        skip_top: strip,
        max_mismatch: MAX_MISMATCH,
    };
    let failure = screenshot_test::check(&shot, |window, cx| {
        super::bind_keys(cx);
        cx.new(|cx| SettingsWindow::new(Category::General, window, cx))
    });
    assert!(failure.is_none(), "{}", failure.unwrap_or_default());
}

/// The Interface page's share of differing pixels: the text, the Skin section, which builds
/// without the `skins` feature leave out (D19) so every row sits higher, and the Hide after
/// slider at 3 s rather than the design's 0.5 s. Measured near 5.3% on macOS (November 2026).
const MAX_INTERFACE_MISMATCH: f64 = 0.065;

#[test]
fn the_interface_page_matches_the_design() {
    let strip = if cfg!(target_os = "macos") { TITLE_BAR_HEIGHT } else { 0. };
    let shot = Shot {
        name: "settings-interface",
        design: "docs/ui/settings/screens/interface/interface_01.png".into(),
        design_origin: DESIGN_ORIGIN,
        window: (DESIGN_SIZE.0, DESIGN_SIZE.1 + strip),
        skip_top: strip,
        max_mismatch: MAX_INTERFACE_MISMATCH,
    };
    let failure = screenshot_test::check(&shot, |window, cx| {
        super::bind_keys(cx);
        cx.new(|cx| SettingsWindow::new(Category::Interface, window, cx))
    });
    assert!(failure.is_none(), "{}", failure.unwrap_or_default());
}

/// The Privacy page's share of differing pixels: the text, the password row's hint (which names
/// the missing keychain where there is none) and the submission URL's default address, which the
/// design leaves empty, and the Advanced Options bar it doesn't draw. Measured near 3.2% on macOS
/// (November 2026).
const MAX_PRIVACY_MISMATCH: f64 = 0.045;

#[test]
fn the_privacy_page_matches_the_design() {
    let strip = if cfg!(target_os = "macos") { TITLE_BAR_HEIGHT } else { 0. };
    let shot = Shot {
        name: "settings-privacy",
        design: "docs/ui/settings/screens/privacy/privacy.png".into(),
        design_origin: DESIGN_ORIGIN,
        window: (DESIGN_SIZE.0, DESIGN_SIZE.1 + strip),
        skip_top: strip,
        max_mismatch: MAX_PRIVACY_MISMATCH,
    };
    let failure = screenshot_test::check(&shot, |window, cx| {
        super::bind_keys(cx);
        cx.new(|cx| SettingsWindow::new(Category::Privacy, window, cx))
    });
    assert!(failure.is_none(), "{}", failure.unwrap_or_default());
}

/// The Controls page's share of differing pixels: the text, the info icons, the two Keyboard
/// Shortcuts buttons, which fit their labels where the design stretches them, and the Remotes
/// rows' info icons and port. Measured near 3.4% on macOS before the Remotes rows (November
/// 2026).
const MAX_CONTROLS_MISMATCH: f64 = 0.045;

#[test]
fn the_controls_page_matches_the_design() {
    let strip = if cfg!(target_os = "macos") { TITLE_BAR_HEIGHT } else { 0. };
    let shot = Shot {
        name: "settings-controls",
        design: "docs/ui/settings/screens/controls/controls.png".into(),
        design_origin: DESIGN_ORIGIN,
        window: (DESIGN_SIZE.0, DESIGN_SIZE.1 + strip),
        skip_top: strip,
        max_mismatch: MAX_CONTROLS_MISMATCH,
    };
    let failure = screenshot_test::check(&shot, |window, cx| {
        super::bind_keys(cx);
        cx.new(|cx| SettingsWindow::new(Category::Controls, window, cx))
    });
    assert!(failure.is_none(), "{}", failure.unwrap_or_default());
}
