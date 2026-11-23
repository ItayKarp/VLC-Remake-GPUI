//! The settings panel's footer (S5.11, S7.1.5): under the Video, Audio and Subtitles tabs, it
//! stays put while they scroll. It holds the preset picker, Save as Default on a tab with session
//! values, and Open Advanced Settings.

use gpui::{App, Div, Entity, Window, div, prelude::*, px};

use super::PanelTab;
use crate::assets::icons;
use crate::effects;
use crate::main_window::bar_panel;
use crate::settings_view::PresetPicker;
use crate::theme::Palette;
use crate::ui::button::{Button, ButtonStyle};

/// The footer's padding, and the space between its controls.
const FOOTER_PADDING: f32 = 20.;
const FOOTER_GAP: f32 = 8.;

/// The footer of the Video, Audio and Subtitles tabs: the preset picker, Save as Default where
/// the tab holds session values, and Open Advanced Settings, which opens `tab`'s page in the
/// Settings window.
pub(super) fn footer(
    tab: PanelTab,
    presets: Entity<PresetPicker>,
    window: &Window,
    p: &Palette,
    cx: &App,
) -> Div {
    let inner = bar_panel::width(window) - 2. * FOOTER_PADDING;
    let save = tab.session_keys().next().is_some().then(|| save_as_default(tab, inner, cx));
    let advanced = Button::new("open-advanced-settings", "Open Advanced Settings")
        .icon(icons::SETTINGS)
        .style(ButtonStyle::Ghost)
        .width(inner)
        .on_click(move |_, _, cx| crate::app::open_settings(Some(tab.category()), cx));
    div()
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(FOOTER_GAP))
        .p(px(FOOTER_PADDING))
        .bg(p.well)
        .child(presets)
        .children(save)
        .child(advanced)
}

/// Save as Default (S7.1.5): writes `tab`'s session values into its Settings page; dimmed while
/// the panel changed nothing there.
fn save_as_default(tab: PanelTab, width: f32, cx: &App) -> Button {
    let changed = tab.session_keys().any(|key| effects::is_changed(cx, key));
    let page = tab.label();
    Button::new("save-as-default", "Save as Default")
        .style(ButtonStyle::Ghost)
        .width(width)
        .disabled(!changed)
        .tooltip(format!("Keep these as Settings → {page}, for later sessions"), None::<&str>)
        .on_click(move |_, _, cx| effects::save_as_default(cx, tab.session_keys()))
}
