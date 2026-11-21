//! Resolved colors. [`Palette::new`] is the single source of colors for the app.
//!
//! Sampled from the designs in `docs/`. Where only one theme has a design (most
//! library, menu and settings surfaces exist only in dark), the other theme's value
//! is derived and marked "derived".

use gpui::{Hsla, rgb};

use super::Accent;
use super::contrast::{self, UI_MIN};

/// Text on accent fills is one of these, whichever contrasts more (S0.3.3).
const ON_ACCENT_DARK: u32 = 0x111111;
const ON_ACCENT_LIGHT: u32 = 0xFFFFFF;

/// Colors for one render pass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub dark: bool,

    pub accent: Hsla,
    /// Text and icons drawn on an accent fill. Meets 4.5:1 against `accent`.
    pub on_accent: Hsla,
    /// The accent for strokes, icons and text drawn on `background`: the accent
    /// itself, or a darker/lighter variant where it would fall below 3:1 (Gray,
    /// Yellow, Teal, Pink and Orange on the light background). Fills behind
    /// `on_accent` keep using `accent`.
    pub accent_fg: Hsla,

    pub background: Hsla,
    /// Recessed areas: text fields, the player preview, seek bar track.
    pub sunken: Hsla,
    pub border: Hsla,
    pub button: Hsla,
    pub button_hover: Hsla,

    pub text_strong: Hsla,
    pub text: Hsla,
    pub text_muted: Hsla,
    pub icon: Hsla,

    pub title_bar: Hsla,
    pub control_bar: Hsla,
    pub volume: Hsla,

    pub key_face: Hsla,
    pub key_edge: Hsla,
    pub key_border: Hsla,

    /// The library's icon rail (`library/sidebar/*`, `player/screens/light`).
    pub sidebar: Hsla,
    /// Library and settings content panels; about 95% opaque over the video.
    pub panel: Hsla,
    /// Selected list item, settings category or sidebar entry.
    pub selected: Hsla,
    /// Hovered option row, menu item or media card.
    pub hover: Hsla,
    /// Hairlines between sections (under the library header).
    pub divider: Hsla,
    /// Menus, dropdowns and popovers. Opaque, unlike the designs' ~95%: they sit
    /// over text, which would show through (S0.4.4c).
    pub menu: Hsla,
    /// Dims the window behind a modal dialog.
    pub scrim: Hsla,
    pub tooltip: Hsla,
    pub tooltip_text: Hsla,
    pub danger: Hsla,
    pub success: Hsla,
    pub warning: Hsla,
    /// Keyboard focus outline.
    pub focus_ring: Hsla,
    /// Letterboxing around video: always black, whatever the theme.
    pub video: Hsla,
    /// Windows' red hover on the title bar Close button, and its icon color.
    pub close_hover: Hsla,
    pub close_hover_icon: Hsla,

    // Component kit (S0.4), sampled from `shared/components/*` and the panels.
    /// The recessed frame around dropdowns, toggles, segmented values and slider
    /// tracks (`#090909` in the dark designs).
    pub well: Hsla,
    /// The raised face inside a well: a closed dropdown, an unselected segment, a
    /// toggle's OFF half.
    pub control: Hsla,
    /// Selected segment, toggle ON track and the filled part of an accent slider:
    /// the accent at about 23% over the well.
    pub accent_tint: Hsla,
    /// Accent text and icons drawn on `accent_tint`; at least 3:1 against it.
    pub accent_on_tint: Hsla,
    /// Toggle ON knob and the accent slider thumb: the accent at 80% over the well.
    pub accent_strong: Hsla,
    /// Toggle OFF knob.
    pub knob: Hsla,
    /// The filled part and the thumb of a neutral slider.
    pub slider_fill: Hsla,
    pub slider_thumb: Hsla,
    /// In-player panels and popovers (`player/panels/*`).
    pub popover: Hsla,
    /// Text selection in inputs.
    pub selection: Hsla,
}

pub(super) fn hex(value: u32) -> Hsla {
    rgb(value).into()
}

fn hexa(value: u32, alpha: f32) -> Hsla {
    Hsla {
        a: alpha,
        ..hex(value)
    }
}

impl Palette {
    pub fn new(dark: bool, accent: Accent) -> Self {
        let accent_color = accent.color();
        let on_accent =
            contrast::readable_on(accent_color, hex(ON_ACCENT_DARK), hex(ON_ACCENT_LIGHT));
        let background = if dark { hex(0x0E0E0E) } else { hex(0xF3F3F3) };
        let accent_fg = contrast::ensure_contrast(accent_color, background, UI_MIN);
        let shared = Shared {
            accent: accent_color,
            on_accent,
            accent_fg,
            background,
        };
        let mut palette = if dark {
            Self::dark(shared)
        } else {
            Self::light(shared)
        };
        palette.derive_accent_tints();
        palette
    }

    /// The accent mixes used by toggles, segmented values and sliders. In the dark
    /// designs the tint is `#371F06` and the knob `#AB6314` for Orange, which is the
    /// accent at 23% and 80% over the well.
    fn derive_accent_tints(&mut self) {
        let base = if self.dark { self.well } else { self.control };
        self.accent_tint = super::blend(self.accent, base, 0.23);
        self.accent_strong = if self.dark {
            super::blend(self.accent, base, 0.8)
        } else {
            self.accent_fg
        };
        self.accent_on_tint = contrast::ensure_contrast(self.accent, self.accent_tint, UI_MIN);
        self.selection = Hsla {
            a: 0.35,
            ..self.accent
        };
    }

    fn dark(s: Shared) -> Self {
        Palette {
            dark: true,
            accent: s.accent,
            on_accent: s.on_accent,
            accent_fg: s.accent_fg,
            background: s.background,
            sunken: hex(0x090909),
            border: hex(0x191919),
            button: hex(0x1B1B1B),
            button_hover: hex(0x242424),
            text_strong: hex(0xE6E6E6),
            text: hex(0xAFAFAF),
            text_muted: hex(0x8F8F8F),
            icon: hex(0x8F8F8F),
            title_bar: hex(0x191919),
            control_bar: hex(0x171717),
            volume: hex(0x7E9D12),
            key_face: hex(0x1B1B1B),
            key_edge: hex(0x222222),
            key_border: hex(0x363636),
            sidebar: hex(0x191919),
            panel: hexa(0x0E0E0E, 0.95),
            selected: hex(0x222222),
            hover: hex(0x1F1F1F),
            divider: hex(0x212121),
            menu: hex(0x181818),
            scrim: hexa(0x000000, 0.6),
            // derived: no tooltip design yet (S0.4.2).
            tooltip: hex(0x2A2A2A),
            tooltip_text: hex(0xE6E6E6),
            // derived: no error/success design yet.
            danger: hex(0xF26D6D),
            success: hex(0x4CC277),
            warning: hex(0xE5A50A),
            focus_ring: s.accent_fg,
            video: hex(0x000000),
            close_hover: hex(0xE81123),
            close_hover_icon: hex(0xFFFFFF),
            well: hex(0x090909),
            control: hex(0x191919),
            knob: hex(0x404040),
            slider_fill: hex(0x232323),
            slider_thumb: hex(0x707070),
            popover: hex(0x1B1B1B),
            // Set by `derive_accent_tints`.
            accent_tint: s.accent,
            accent_on_tint: s.accent,
            accent_strong: s.accent,
            selection: s.accent,
        }
    }

    fn light(s: Shared) -> Self {
        Palette {
            dark: false,
            accent: s.accent,
            on_accent: s.on_accent,
            accent_fg: s.accent_fg,
            background: s.background,
            sunken: hex(0xFFFFFF),
            border: hex(0xDCDCDC),
            button: hex(0xE3E3E3),
            button_hover: hex(0xD8D8D8),
            text_strong: hex(0x161616),
            text: hex(0x4A4A4A),
            text_muted: hex(0x6B6B6B),
            icon: hex(0x6B6B6B),
            title_bar: hex(0xE6E6E6),
            control_bar: hex(0xEAEAEA),
            volume: hex(0x7E9D12),
            key_face: hex(0xFFFFFF),
            key_edge: hex(0xD6D6D6),
            key_border: hex(0xC8C8C8),
            // `player/screens/light/player_light.png`. This design is much darker
            // than onboarding's light preview (title bar #ABABAA there versus
            // #E6E6E6 here); see PRD Appendix C.
            sidebar: hex(0xABABAA),
            panel: hexa(0xC2C2C1, 0.95),
            // derived from the dark design's steps.
            selected: hex(0xDCDCDC),
            hover: hex(0xE6E6E6),
            divider: hex(0x717171),
            // derived
            menu: hex(0xFAFAFA),
            scrim: hexa(0x000000, 0.4),
            tooltip: hex(0x2A2A2A),
            tooltip_text: hex(0xF3F3F3),
            danger: hex(0xC42B2B),
            success: hex(0x1E7A3C),
            warning: hex(0x8A5A00),
            focus_ring: s.accent_fg,
            video: hex(0x000000),
            close_hover: hex(0xE81123),
            close_hover_icon: hex(0xFFFFFF),
            // derived: the kit's controls have dark designs only. The light theme
            // inverts the well/face relationship: a gray frame around a white face.
            well: hex(0xD6D6D6),
            control: hex(0xFFFFFF),
            knob: hex(0xB4B4B4),
            slider_fill: hex(0xBDBDBD),
            slider_thumb: hex(0x7A7A7A),
            popover: hex(0xFAFAFA),
            // Set by `derive_accent_tints`.
            accent_tint: s.accent,
            accent_on_tint: s.accent,
            accent_strong: s.accent,
            selection: s.accent,
        }
    }

    /// Windows High Contrast / forced colors (S0.3.6): every token maps to one of
    /// the user's system colors, and the theme and accent are ignored.
    pub fn high_contrast(sys: SystemColors) -> Self {
        Palette {
            dark: contrast::luminance(sys.window) < 0.18,
            accent: sys.highlight,
            on_accent: sys.highlight_text,
            accent_fg: sys.hotlight,
            background: sys.window,
            sunken: sys.window,
            border: sys.window_text,
            button: sys.button_face,
            button_hover: sys.window,
            text_strong: sys.window_text,
            text: sys.window_text,
            text_muted: sys.window_text,
            icon: sys.window_text,
            title_bar: sys.window,
            control_bar: sys.window,
            volume: sys.highlight,
            key_face: sys.button_face,
            key_edge: sys.window_text,
            key_border: sys.window_text,
            sidebar: sys.window,
            panel: sys.window,
            selected: sys.button_face,
            hover: sys.button_face,
            divider: sys.window_text,
            menu: sys.window,
            scrim: hexa(0x000000, 0.6),
            tooltip: sys.window,
            tooltip_text: sys.window_text,
            danger: sys.window_text,
            success: sys.window_text,
            warning: sys.window_text,
            focus_ring: sys.window_text,
            video: hex(0x000000),
            close_hover: sys.highlight,
            close_hover_icon: sys.highlight_text,
            well: sys.window_text,
            control: sys.button_face,
            accent_tint: sys.highlight,
            accent_on_tint: sys.highlight_text,
            accent_strong: sys.highlight_text,
            knob: sys.window_text,
            slider_fill: sys.highlight,
            slider_thumb: sys.window_text,
            popover: sys.window,
            selection: sys.highlight,
        }
    }
}

struct Shared {
    accent: Hsla,
    on_accent: Hsla,
    accent_fg: Hsla,
    background: Hsla,
}

/// The OS's high-contrast colors (Windows `GetSysColor`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SystemColors {
    pub window: Hsla,
    pub window_text: Hsla,
    pub highlight: Hsla,
    pub highlight_text: Hsla,
    pub hotlight: Hsla,
    pub button_face: Hsla,
}
