//! The app-wide theme service (S0.3).
//!
//! The [`Theme`] global holds the theme mode and accent, mirrored from settings
//! (which own and persist them), plus the OS accessibility preferences. Views call
//! [`observe`] once and [`palette`] in `render`; they repaint when the theme, the
//! accent, the OS light/dark appearance or high contrast changes.
//!
//! [`Palette::new`] is the single source of colors.

pub mod contrast;
mod legibility;
mod palette;
mod system;
#[cfg(test)]
mod tests;
mod text_scale;

use gpui::{
    App, AppContext, BorrowAppContext, Context, Global, Hsla, Rgba, Window, WindowAppearance, px,
    rgb,
};
use serde::{Deserialize, Serialize};

use crate::settings::{self, SettingsStore};
pub use palette::Palette;
pub use system::SystemPrefs;
pub use text_scale::text_px;

/// Theme mode the user picks during onboarding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    /// Always light.
    Light,
    /// Always dark.
    #[default]
    Dark,
    /// Follows the OS appearance.
    System,
}

impl ThemeMode {
    /// Every mode, in the order every theme picker shows them (D6: Light, Dark, System).
    pub const ALL: [ThemeMode; 3] = [ThemeMode::Light, ThemeMode::Dark, ThemeMode::System];

    /// The name shown to the user.
    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::Light => tr!("Light"),
            ThemeMode::Dark => tr!("Dark"),
            ThemeMode::System => tr!("System"),
        }
    }

    /// A stable lowercase identifier, as written in the settings file.
    pub fn key(self) -> &'static str {
        match self {
            ThemeMode::Light => "light",
            ThemeMode::Dark => "dark",
            ThemeMode::System => "system",
        }
    }

    /// Whether this mode renders dark, given the OS appearance. "System" follows
    /// the OS; GPUI reports dark on Linux desktops that don't expose an appearance.
    pub fn is_dark(self, appearance: WindowAppearance) -> bool {
        match self {
            ThemeMode::Light => false,
            ThemeMode::Dark => true,
            ThemeMode::System => {
                matches!(appearance, WindowAppearance::Dark | WindowAppearance::VibrantDark)
            }
        }
    }
}

/// The accent colors offered on the "theme and color" onboarding step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Accent {
    /// VLC's orange, the default.
    #[default]
    Orange,
    /// Red.
    Red,
    /// Yellow.
    Yellow,
    /// Green.
    Green,
    /// Teal.
    Teal,
    /// Blue.
    Blue,
    /// Purple.
    Purple,
    /// Pink.
    Pink,
    /// Neutral gray.
    Gray,
}

impl Accent {
    /// Every accent, in the order onboarding shows them.
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

    /// The accent's color, from the designs.
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

    /// A stable lowercase identifier, as written in the settings file.
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

/// The app-wide theme. Read it with [`palette`]; change the mode and accent through
/// settings ([`settings::update`]), which this mirrors.
pub struct Theme {
    mode: ThemeMode,
    accent: Accent,
    system: SystemPrefs,
}

impl Global for Theme {}

impl Theme {
    /// The palette for the saved theme, given a window's appearance.
    pub fn palette(&self, appearance: WindowAppearance) -> Palette {
        self.palette_for(self.mode, self.accent, appearance)
    }

    /// The palette for a theme that isn't saved yet (onboarding's live preview).
    /// High contrast still wins.
    pub fn palette_for(
        &self,
        mode: ThemeMode,
        accent: Accent,
        appearance: WindowAppearance,
    ) -> Palette {
        match self.system.high_contrast {
            Some(colors) => Palette::high_contrast(colors),
            None => Palette::new(mode.is_dark(appearance), accent),
        }
    }
}

/// Installs the global. Call after [`settings::init`] and before opening windows.
pub fn init(cx: &mut App) {
    let prefs = &settings::get(cx).prefs;
    let (mode, accent) = (prefs.theme, prefs.accent);
    let system = if system::READ_IS_CHEAP { system::read() } else { SystemPrefs::default() };
    cx.set_reduce_motion(system.reduce_motion);
    text_scale::set(system.text_scale);
    cx.set_global(Theme { mode, accent, system });
    cx.observe_global::<SettingsStore>(sync_from_settings).detach();
    if !system::READ_IS_CHEAP {
        refresh_system_prefs(cx);
    }
}

/// Copies the mode and accent from settings. Only touches the global when they
/// changed, so saving unrelated settings (window geometry) doesn't repaint.
fn sync_from_settings(cx: &mut App) {
    let prefs = &settings::get(cx).prefs;
    let (mode, accent) = (prefs.theme, prefs.accent);
    let theme = cx.global::<Theme>();
    if theme.mode != mode || theme.accent != accent {
        cx.update_global::<Theme, _>(|theme, _| {
            theme.mode = mode;
            theme.accent = accent;
        });
    }
}

/// Re-reads the OS accessibility preferences. Windows doesn't tell windows when
/// high contrast or animations are toggled, so this runs whenever a window is
/// activated or the OS appearance changes.
pub fn refresh_system_prefs(cx: &mut App) {
    if system::READ_IS_CHEAP {
        set_system_prefs(system::read(), cx);
    } else {
        cx.spawn(async move |cx| {
            let prefs = cx.background_spawn(async { system::read() }).await;
            cx.update(|cx| set_system_prefs(prefs, cx));
        })
        .detach();
    }
}

fn set_system_prefs(prefs: SystemPrefs, cx: &mut App) {
    if cx.global::<Theme>().system == prefs {
        return;
    }
    tracing::info!("theme: OS preferences changed: {prefs:?}");
    cx.set_reduce_motion(prefs.reduce_motion);
    text_scale::set(prefs.text_scale);
    cx.update_global::<Theme, _>(|theme, _| theme.system = prefs);
    // Text sizes are read while views render, so every view draws again at the new size.
    cx.refresh_windows();
}

/// The current palette for `window`.
pub fn palette(window: &Window, cx: &App) -> Palette {
    cx.global::<Theme>().palette(window.appearance())
}

/// Repaints a view when the theme, the accent, the OS appearance or the OS
/// accessibility preferences change, and keeps the window's rem at the OS text size. Call
/// once from the view's constructor.
pub fn observe<V: 'static>(window: &mut Window, cx: &mut Context<V>) {
    follow_text_scale(window);
    cx.observe_global_in::<Theme>(window, |_, window, cx| {
        follow_text_scale(window);
        cx.notify();
    })
    .detach();
    cx.observe_window_appearance(window, |_, _, cx| {
        refresh_system_prefs(cx);
        cx.notify();
    })
    .detach();
    cx.observe_window_activation(window, |_, window, cx| {
        if window.is_window_active() {
            refresh_system_prefs(cx);
        }
    })
    .detach();
}

/// Sets the rem, which text drawn at the default size follows, to the OS text size.
fn follow_text_scale(window: &mut Window) {
    window.set_rem_size(px(text_scale::BASE_REM * text_scale::scale()));
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
