//! Keeps a palette's text, icons, accent strokes and focus ring legible on every
//! surface of its theme (S9.1.3).
//!
//! The designs' colors pass on the window background but not everywhere: the light
//! player design's sidebar (`#ABABAA`) and panels (`#C2C2C1`) are mid-grays that
//! muted text and most accents fall below the minimum on, and the dark raised
//! surfaces are too light for Blue. Each foreground token is moved toward black (light
//! theme) or white (dark theme) just far enough to pass on all of them.

use gpui::Hsla;

use super::Palette;
use super::contrast::{TEXT_MIN, UI_MIN, ensure_contrast};

impl Palette {
    /// The opaque surfaces text, icons and focus rings are drawn on. Panels are
    /// translucent, so they're counted over both the window and the video.
    pub fn surfaces(&self) -> Vec<Hsla> {
        let panel_alpha = self.panel.a;
        vec![
            self.background,
            super::blend(self.panel, self.background, panel_alpha),
            super::blend(self.panel, self.video, panel_alpha),
            self.sidebar,
            self.menu,
            self.popover,
            self.selected,
            self.hover,
            self.control,
            self.well,
            self.title_bar,
            self.control_bar,
            self.sunken,
            self.button,
            self.button_hover,
        ]
    }

    /// Moves every foreground token far enough to pass on [`Self::surfaces`]: body
    /// text and status colors at 4.5:1, icons, accent strokes and the focus ring at
    /// 3:1.
    pub(super) fn meet_contrast(&mut self) {
        let surfaces = self.surfaces();
        let legible = |color, min| on_all(color, &surfaces, min);
        self.text_strong = legible(self.text_strong, TEXT_MIN);
        self.text = legible(self.text, TEXT_MIN);
        self.text_muted = legible(self.text_muted, TEXT_MIN);
        self.danger = legible(self.danger, TEXT_MIN);
        self.success = legible(self.success, TEXT_MIN);
        self.warning = legible(self.warning, TEXT_MIN);
        self.icon = legible(self.icon, UI_MIN);
        self.accent_fg = legible(self.accent_fg, UI_MIN);
        self.focus_ring = legible(self.focus_ring, UI_MIN);
    }
}

/// `color`, moved just far enough to reach `min` against every surface. All of a
/// theme's surfaces sit on the same side of mid-gray, so each step only moves the
/// color further from all of them and never undoes an earlier one.
fn on_all(color: Hsla, surfaces: &[Hsla], min: f32) -> Hsla {
    surfaces.iter().fold(color, |color, surface| ensure_contrast(color, *surface, min))
}
