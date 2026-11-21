//! Colors for the redesigned UI, sampled from the design references in `docs/`.

use gpui::{Hsla, Rgba, WindowAppearance, rgb};
use serde::{Deserialize, Serialize};

/// Theme mode the user picks during onboarding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Light,
    #[default]
    Dark,
    System,
}

impl ThemeMode {
    pub const ALL: [ThemeMode; 3] = [ThemeMode::Light, ThemeMode::Dark, ThemeMode::System];

    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::Light => "Light",
            ThemeMode::Dark => "Dark",
            ThemeMode::System => "System",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            ThemeMode::Light => "light",
            ThemeMode::Dark => "dark",
            ThemeMode::System => "system",
        }
    }

    /// Whether this mode renders dark, given the OS appearance.
    pub fn is_dark(self, appearance: WindowAppearance) -> bool {
        match self {
            ThemeMode::Light => false,
            ThemeMode::Dark => true,
            ThemeMode::System => matches!(
                appearance,
                WindowAppearance::Dark | WindowAppearance::VibrantDark
            ),
        }
    }
}

/// The accent colors offered on the "theme and color" onboarding step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Accent {
    #[default]
    Orange,
    Red,
    Yellow,
    Green,
    Teal,
    Blue,
    Purple,
    Pink,
    Gray,
}

impl Accent {
    pub const ALL: [Accent; 9] = [
        Accent::Orange,
        Accent::Red,
        Accent::Yellow,
        Accent::Green,
        Accent::Teal,
        Accent::Blue,
        Accent::Purple,
        Accent::Pink,
        Accent::Gray,
    ];

    pub fn color(self) -> Hsla {
        let hex = match self {
            Accent::Orange => 0xD37918,
            Accent::Red => 0xE15032,
            Accent::Yellow => 0xC49E23,
            Accent::Green => 0x19A234,
            Accent::Teal => 0x18AAD3,
            Accent::Blue => 0x185DD3,
            Accent::Purple => 0x8E42FF,
            Accent::Pink => 0xF052B3,
            Accent::Gray => 0xAFAFAF,
        };
        rgb(hex).into()
    }

    pub fn key(self) -> &'static str {
        match self {
            Accent::Orange => "orange",
            Accent::Red => "red",
            Accent::Yellow => "yellow",
            Accent::Green => "green",
            Accent::Teal => "teal",
            Accent::Blue => "blue",
            Accent::Purple => "purple",
            Accent::Pink => "pink",
            Accent::Gray => "gray",
        }
    }
}

/// Resolved colors for one render pass.
#[derive(Clone, Copy)]
pub struct Palette {
    pub accent: Hsla,
    /// Text drawn on top of an accent-filled button.
    pub on_accent: Hsla,

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
}

fn hex(value: u32) -> Hsla {
    rgb(value).into()
}

impl Palette {
    pub fn new(dark: bool, accent: Accent) -> Self {
        let accent_color = accent.color();
        let on_accent = hex(0x111111);
        if dark {
            Palette {
                accent: accent_color,
                on_accent,
                background: hex(0x0E0E0E),
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
            }
        } else {
            Palette {
                accent: accent_color,
                on_accent,
                background: hex(0xF3F3F3),
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
            }
        }
    }
}

/// Blend `color` over `base` at the given opacity — used for the muted
/// (unselected) swatches and selection rings, which are opaque in the design.
pub fn blend(color: Hsla, base: Hsla, alpha: f32) -> Hsla {
    let c: Rgba = color.into();
    let b: Rgba = base.into();
    Rgba {
        r: c.r * alpha + b.r * (1.0 - alpha),
        g: c.g * alpha + b.g * (1.0 - alpha),
        b: c.b * alpha + b.b * (1.0 - alpha),
        a: 1.0,
    }
    .into()
}
