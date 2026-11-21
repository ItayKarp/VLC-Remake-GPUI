//! WCAG 2.x contrast helpers (S0.3.3).

use gpui::{Hsla, Rgba};

/// Minimum contrast for body text (WCAG AA, 1.4.3).
#[cfg_attr(not(test), allow(dead_code, reason = "checked by the tests"))]
pub const TEXT_MIN: f32 = 4.5;
/// Minimum contrast for UI components, strokes and icons (WCAG AA, 1.4.11).
pub const UI_MIN: f32 = 3.0;

/// Relative luminance of an opaque color, 0.0 (black) to 1.0 (white).
pub fn luminance(color: Hsla) -> f32 {
    fn channel(c: f32) -> f32 {
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    }
    let c: Rgba = color.into();
    0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b)
}

/// Contrast ratio between two opaque colors, 1.0 to 21.0.
pub fn ratio(a: Hsla, b: Hsla) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Whichever of `dark` and `light` contrasts more with `background`.
pub fn readable_on(background: Hsla, dark: Hsla, light: Hsla) -> Hsla {
    if ratio(background, dark) >= ratio(background, light) { dark } else { light }
}

/// `color`, moved toward black (on light backgrounds) or white (on dark ones) just
/// far enough to reach `min` contrast with `background`. Returns `color` unchanged
/// when it already passes.
pub fn ensure_contrast(color: Hsla, background: Hsla, min: f32) -> Hsla {
    if ratio(color, background) >= min {
        return color;
    }
    let target = if luminance(background) > 0.18 { gpui::black() } else { gpui::white() };
    // Blending to pure black/white always ends at ≥ 4.5:1 against any background on
    // the far side of mid-gray, so 20 steps of 5% always reach `min` ≤ 4.5.
    (1..=20)
        .map(|step| super::blend(target, color, step as f32 * 0.05))
        .find(|candidate| ratio(*candidate, background) >= min)
        .unwrap_or(target)
}
