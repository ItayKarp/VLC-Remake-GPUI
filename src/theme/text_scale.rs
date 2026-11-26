//! The OS text size (S9.1.4): Windows' "Make text bigger" and GNOME's text scaling factor.
//! macOS has no system-wide text size for apps, so there it stays 1.
//!
//! Text sizes are written as pixels all over the views, in builders that have no `App` to ask,
//! so the factor is kept in a static that [`text_px`] reads; [`super::set_system_prefs`] sets it
//! with the other OS preferences and repaints every window. Text drawn at the default size
//! follows the window's rem, which [`super::observe`] keeps at [`BASE_REM`] times the factor.
//! Only text grows: row heights, icons and spacing keep their design sizes.

use std::sync::atomic::{AtomicU32, Ordering};

use gpui::{Pixels, px};

/// GPUI's rem before scaling.
pub const BASE_REM: f32 = 16.;
/// The smallest and largest factors followed. Windows offers 100–225%; GNOME any factor, but
/// below this text stops being legible and above it labels no longer fit their rows at all.
const MIN: f32 = 0.75;
const MAX: f32 = 2.;

/// The factor, as `f32` bits; 1.0 until the OS is read.
static SCALE: AtomicU32 = AtomicU32::new(0x3F80_0000);

/// `size` design pixels of text, at the OS text size.
pub fn text_px(size: f32) -> Pixels {
    px(size * scale())
}

/// The factor text is drawn at.
pub fn scale() -> f32 {
    f32::from_bits(SCALE.load(Ordering::Relaxed))
}

/// Follows `factor` from now on.
pub(super) fn set(factor: f32) {
    SCALE.store(factor.to_bits(), Ordering::Relaxed);
}

/// A factor the OS reported, kept within what the layout survives; 1 for nonsense (not a
/// number, zero or negative).
pub fn clamp(factor: f32) -> f32 {
    if factor.is_finite() && factor > 0. { factor.clamp(MIN, MAX) } else { 1. }
}

/// The variable debug builds take the factor from, to try larger text where the OS has no
/// setting for it (macOS): `RVLC_TEXT_SCALE=1.5`.
const DEBUG_VARIABLE: &str = "RVLC_TEXT_SCALE";

/// The factor `RVLC_TEXT_SCALE` asks for, in debug builds.
pub fn debug_override() -> Option<f32> {
    if !cfg!(debug_assertions) {
        return None;
    }
    std::env::var(DEBUG_VARIABLE).ok().map(|value| parse_gsettings(&value))
}

/// GNOME's `text-scaling-factor` as `gsettings` prints it ("1.25"), or the debug override.
pub fn parse_gsettings(output: &str) -> f32 {
    output.trim().parse().map_or(1., clamp)
}

/// Windows' `TextScaleFactor` registry value, a percentage from 100 to 225.
#[cfg_attr(not(target_os = "windows"), allow(dead_code, reason = "Windows reads the registry"))]
pub fn from_percent(percent: u32) -> f32 {
    clamp(percent as f32 / 100.)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gsettings_factors_are_read_and_bounded() {
        assert!((parse_gsettings("1.25\n") - 1.25).abs() < f32::EPSILON);
        assert!((parse_gsettings("5.0") - MAX).abs() < f32::EPSILON);
        assert!((parse_gsettings("0.1") - MIN).abs() < f32::EPSILON);
        assert!((parse_gsettings("") - 1.).abs() < f32::EPSILON);
        assert!((parse_gsettings("nan") - 1.).abs() < f32::EPSILON);
    }

    #[test]
    fn windows_percentages_become_factors() {
        assert!((from_percent(100) - 1.).abs() < f32::EPSILON);
        assert!((from_percent(150) - 1.5).abs() < f32::EPSILON);
        assert!((from_percent(225) - MAX).abs() < f32::EPSILON);
        assert!((from_percent(0) - 1.).abs() < f32::EPSILON);
    }
}
