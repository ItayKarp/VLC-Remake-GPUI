//! Tests for the palettes: contrast targets for every accent and theme, high contrast,
//! and theme mode resolution.

use gpui::WindowAppearance;

use super::contrast::{self, TEXT_MIN, UI_MIN, ratio};
use super::palette::SystemColors;
use super::palette::hex;
use super::{Accent, Palette, ThemeMode};

#[test]
fn on_accent_meets_aa_for_every_accent_and_theme() {
    for dark in [false, true] {
        for accent in Accent::ALL {
            let p = Palette::new(dark, accent);
            let r = ratio(p.on_accent, p.accent);
            assert!(
                r >= TEXT_MIN,
                "{accent:?} (dark: {dark}): on_accent contrast {r:.2} < {TEXT_MIN}"
            );
        }
    }
}

#[test]
fn on_accent_picks_white_where_dark_text_fails() {
    // The PRD's examples: #111111 is 3.19:1 on Blue and 3.93:1 on Purple, white
    // is 5.92:1 and 4.80:1.
    for accent in [Accent::Blue, Accent::Purple] {
        assert_eq!(Palette::new(true, accent).on_accent, hex(0xFFFFFF));
    }
    assert!((ratio(hex(0x111111), Accent::Blue.color()) - 3.19).abs() < 0.02);
    assert!((ratio(hex(0xFFFFFF), Accent::Purple.color()) - 4.80).abs() < 0.02);
    // The design's dark text stays on Orange.
    assert_eq!(Palette::new(true, Accent::Orange).on_accent, hex(0x111111));
}

#[test]
fn accent_fg_meets_ui_minimum_on_background() {
    for dark in [false, true] {
        for accent in Accent::ALL {
            let p = Palette::new(dark, accent);
            let r = ratio(p.accent_fg, p.background);
            assert!(r >= UI_MIN, "{accent:?} (dark: {dark}): accent_fg contrast {r:.2} < {UI_MIN}");
        }
    }
}

#[test]
fn accent_fg_is_the_accent_when_it_already_passes() {
    let p = Palette::new(false, Accent::Blue);
    assert_eq!(p.accent_fg, p.accent);
    // Gray is 1.98:1 on the light background, so it's darkened.
    let p = Palette::new(false, Accent::Gray);
    assert_ne!(p.accent_fg, p.accent);
    assert!(contrast::luminance(p.accent_fg) < contrast::luminance(p.accent));
}

#[test]
fn text_meets_aa_on_background() {
    for dark in [false, true] {
        let p = Palette::new(dark, Accent::Orange);
        for (name, color) in [
            ("text_strong", p.text_strong),
            ("text", p.text),
            ("text_muted", p.text_muted),
            ("tooltip_text", p.tooltip_text),
        ] {
            let bg = if name == "tooltip_text" { p.tooltip } else { p.background };
            let r = ratio(color, bg);
            assert!(r >= TEXT_MIN, "{name} (dark: {dark}): {r:.2}");
        }
        for (name, color) in [("danger", p.danger), ("success", p.success)] {
            let r = ratio(color, p.background);
            assert!(r >= TEXT_MIN, "{name} (dark: {dark}): {r:.2}");
        }
    }
}

#[test]
fn ensure_contrast_lightens_on_dark_backgrounds() {
    let bg = hex(0x0E0E0E);
    let dim = hex(0x303030);
    let fixed = contrast::ensure_contrast(dim, bg, UI_MIN);
    assert!(ratio(fixed, bg) >= UI_MIN);
    assert!(contrast::luminance(fixed) > contrast::luminance(dim));
}

#[test]
fn high_contrast_ignores_theme_and_accent() {
    let sys = SystemColors {
        window: hex(0x000000),
        window_text: hex(0xFFFFFF),
        highlight: hex(0x1AEBFF),
        highlight_text: hex(0x000000),
        hotlight: hex(0xFFFF00),
        button_face: hex(0x000000),
    };
    let theme = super::Theme {
        mode: ThemeMode::Light,
        accent: Accent::Pink,
        system: super::SystemPrefs { reduce_motion: false, high_contrast: Some(sys) },
    };
    let p = theme.palette(WindowAppearance::Light);
    assert_eq!(p, Palette::high_contrast(sys));
    assert!(p.dark);
    assert_eq!(p.background, sys.window);
    assert_eq!(p.accent, sys.highlight);
    // Video letterboxing stays black in every theme.
    assert_eq!(p.video, hex(0x000000));
}

#[test]
fn system_mode_follows_appearance() {
    assert!(ThemeMode::System.is_dark(WindowAppearance::Dark));
    assert!(ThemeMode::System.is_dark(WindowAppearance::VibrantDark));
    assert!(!ThemeMode::System.is_dark(WindowAppearance::Light));
    assert!(ThemeMode::Dark.is_dark(WindowAppearance::Light));
    assert!(!ThemeMode::Light.is_dark(WindowAppearance::Dark));
}
