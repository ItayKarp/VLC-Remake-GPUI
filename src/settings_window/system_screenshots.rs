//! Screenshot tests of the Network, Performance and Recording pages against
//! `settings/screens/network/network.png`, `settings/screens/performance/performance.png` and
//! `settings/screens/recording/recording.png`.

use gpui::AppContext as _;

use super::SettingsWindow;
use crate::screenshot_test::{self, Shot};
use crate::settings::registry::Category;
use crate::window_chrome::TITLE_BAR_HEIGHT;

/// The window inside the design PNG's drop shadow, in the PNG's pixels and in logical pixels.
const DESIGN_ORIGIN: (u32, u32) = (30, 30);
const DESIGN_SIZE: (f32, f32) = (1048., 860.);

/// Checks `category`'s page against `design`, allowing `max_mismatch` of the pixels to differ.
fn check(category: Category, name: &'static str, design: &str, max_mismatch: f64) {
    // macOS gets a strip for the traffic lights, which the design leaves out.
    let strip = if cfg!(target_os = "macos") { TITLE_BAR_HEIGHT } else { 0. };
    let shot = Shot {
        name,
        design: design.into(),
        design_origin: DESIGN_ORIGIN,
        window: (DESIGN_SIZE.0, DESIGN_SIZE.1 + strip),
        skip_top: strip,
        max_mismatch,
    };
    let failure = screenshot_test::check(&shot, |window, cx| {
        super::bind_keys(cx);
        cx.new(|cx| SettingsWindow::new(category, window, cx))
    });
    assert!(failure.is_none(), "{}", failure.unwrap_or_default());
}

/// The Network page's share of differing pixels: the text, and the dimmed buffering fields,
/// which show their times where the design leaves them empty, the proxy's Test button and
/// the password field's "Not saved". Measured near 2.7% on macOS (November 2026).
const MAX_NETWORK_MISMATCH: f64 = 0.04;

#[test]
fn the_network_page_matches_the_design() {
    check(
        Category::Network,
        "settings-network",
        "docs/ui/settings/screens/network/network.png",
        MAX_NETWORK_MISMATCH,
    );
}

/// The Performance page's share of differing pixels: the text and number fields' units, and
/// Advanced Options, closed here where the design shows it open, and the Rescan button, which
/// fits its label; on macOS also the real-time and process priority rows, which its libVLC
/// doesn't have, moving Rescan plugins up. Measured near 3.5% on macOS (November 2026).
const MAX_PERFORMANCE_MISMATCH: f64 = 0.05;

#[test]
fn the_performance_page_matches_the_design() {
    check(
        Category::Performance,
        "settings-performance",
        "docs/ui/settings/screens/performance/performance.png",
        MAX_PERFORMANCE_MISMATCH,
    );
}

/// The Recording page's share of differing pixels: the text, the info icons the design leaves
/// off the toggles, the recordings folder's placeholder, and Advanced Options, which the page
/// doesn't have. Measured near 4.1% on macOS (November 2026).
const MAX_RECORDING_MISMATCH: f64 = 0.055;

#[test]
fn the_recording_page_matches_the_design() {
    check(
        Category::Recording,
        "settings-recording",
        "docs/ui/settings/screens/recording/recording.png",
        MAX_RECORDING_MISMATCH,
    );
}
