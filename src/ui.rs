//! The shared component kit (S0.4).
//!
//! Every screen builds from these pieces so popovers, menus, dropdowns, sliders and
//! panel sections look and behave the same everywhere. They follow
//! `docs/ui/shared/components/*`, `docs/ui/settings/components/*` and
//! `docs/ui/library/menus/*`, sampled at 1x (the PNGs are 2x).
//!
//! - Stateless pieces are `RenderOnce` builders: [`Button`], [`IconButton`],
//!   [`Toggle`], [`Checkbox`], [`Segmented`], [`StackedChoice`], [`Tabs`],
//!   [`OptionRow`], [`Collapsible`], [`banner`].
//! - Pieces with their own state are entities: [`Dropdown`], [`ContextMenu`],
//!   [`Slider`], [`TextInput`], [`Toasts`], and the [`ModalLayer`].
//! - Every control is keyboard reachable and shows a focus ring for keyboard focus
//!   ([`focus`]).
//!
//! **Translucent surfaces (S0.4.4c).** Menus, popovers, panels and tooltips are
//! about 95% opaque in the designs. They use flat colors at that opacity
//! (`Palette::menu`, `Palette::panel`), not OS backdrop blur: GPUI can only blur
//! the whole window background (`WindowBackgroundAppearance::Blurred`), not one
//! element, and Mica/Acrylic/vibrancy behind a video would show the desktop, not
//! the video. At 95% the difference from real blur isn't visible.

pub mod button;
pub mod collapsible;
pub mod drop_target;
pub mod dropdown;
pub mod focus;
pub mod gallery;
pub mod icon;
pub mod list;
pub mod menu;
pub mod modal;
pub mod option_row;
pub mod popover;
pub mod segmented;
pub mod slider;
pub mod tabs;
#[cfg(test)]
mod tests;
pub mod text_input;
#[allow(dead_code, reason = "the library views (S4.6) are its first callers")]
pub mod thumbnail;
pub mod toast;
pub mod toggle;
pub mod tooltip;

use gpui::{App, KeyBinding};

/// Sizes shared by the components, in logical pixels (the designs at 1x).
pub mod metrics {
    /// Buttons (`shared/components/button-*`: 360×32).
    pub const BUTTON_HEIGHT: f32 = 32.;
    /// Dropdowns, option rows and segmented values (`dropdown-*`, `option-*`,
    /// `values-*`: 44–48 tall).
    pub const FIELD_HEIGHT: f32 = 44.;
    /// Panel section headers (`video_colors-collapsed`: 360×48).
    pub const SECTION_HEIGHT: f32 = 48.;
    /// Corner radius of controls.
    pub const RADIUS: f32 = 4.;
    /// Corner radius of floating surfaces: menus, popovers, toasts.
    pub const RADIUS_LARGE: f32 = 8.;
    /// The well's inset around a control's face.
    pub const WELL_INSET: f32 = 2.;
    /// Body text size.
    pub const TEXT: f32 = 16.;
    /// Secondary text size: hints and shortcuts.
    pub const TEXT_SMALL: f32 = 13.;
    /// Icon size in controls.
    pub const ICON: f32 = 20.;
    /// Space kept between floating surfaces and the window edges.
    pub const EDGE_MARGIN: f32 = 8.;
}

/// Key bindings for the kit. Call once at startup.
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", focus::FocusNext, None),
        KeyBinding::new("shift-tab", focus::FocusPrev, None),
    ]);
    text_input::bind_keys(cx);
}
