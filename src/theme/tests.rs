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
    let p = Palette::new(true, Accent::Orange);
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
        system: super::SystemPrefs { high_contrast: Some(sys), ..super::SystemPrefs::default() },
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

/// Every foreground token against every surface, for both themes and every accent
/// (S9.1.3): text 4.5:1; icons, accent strokes and the focus ring 3:1.
#[test]
fn foregrounds_meet_wcag_on_every_surface() {
    for dark in [false, true] {
        for accent in Accent::ALL {
            let p = Palette::new(dark, accent);
            let foregrounds = [
                ("text_strong", p.text_strong, TEXT_MIN),
                ("text", p.text, TEXT_MIN),
                ("text_muted", p.text_muted, TEXT_MIN),
                ("danger", p.danger, TEXT_MIN),
                ("success", p.success, TEXT_MIN),
                ("warning", p.warning, TEXT_MIN),
                ("icon", p.icon, UI_MIN),
                ("accent_fg", p.accent_fg, UI_MIN),
                ("focus_ring", p.focus_ring, UI_MIN),
            ];
            for (name, color, min) in foregrounds {
                assert_on_every_surface(&p, color, min, &format!("{accent:?} {name}"));
            }
        }
    }
}

/// Fails, naming `what` and the surface, where `color` is below `min` on one of `p`'s surfaces.
fn assert_on_every_surface(p: &Palette, color: gpui::Hsla, min: f32, what: &str) {
    for (index, surface) in p.surfaces().into_iter().enumerate() {
        let r = ratio(color, surface);
        assert!(r >= min, "{what} (dark: {}) on surface {index}: {r:.2}", p.dark);
    }
}

#[test]
fn legibility_keeps_tokens_that_already_pass() {
    // The dark design's body text passes everywhere, so it's left as sampled.
    let p = Palette::new(true, Accent::Orange);
    assert_eq!(p.text_strong, hex(0xE6E6E6));
    // Light muted text fails on the gray sidebar, so it's darkened.
    let light = Palette::new(false, Accent::Orange);
    assert!(contrast::luminance(light.text_muted) < contrast::luminance(hex(0x6B6B6B)));
}
