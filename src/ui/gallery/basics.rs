//! The gallery's layout helpers and its stateless sections: theme switches and buttons.

use gpui::{App, Div, div, prelude::*, px};

use crate::assets::{icons, two_tone};
use crate::settings;
use crate::theme::text_px;
use crate::theme::{Accent, Palette, ThemeMode};
use crate::ui::button::{Button, ButtonStyle, IconButton};
use crate::ui::icon;
use crate::ui::segmented::Segmented;

/// The size two-tone icons are shown at, next to the icon buttons.
const TWO_TONE_SIZE: f32 = 24.;

/// A titled panel holding one group of components.
pub(super) fn section(title: &'static str, p: &Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .p(px(16.))
        .rounded(px(10.))
        .bg(p.panel)
        .border_1()
        .border_color(p.divider)
        .child(div().text_size(text_px(12.)).text_color(p.text_muted).child(title.to_uppercase()))
}

/// A wrapping row of components inside a section.
pub(super) fn row() -> Div {
    div().flex().flex_row().flex_wrap().items_center().gap(px(12.))
}

/// The theme and accent switches, which change the real settings.
pub(super) fn appearance(p: &Palette, cx: &App) -> Div {
    let prefs = &settings::get(cx).prefs;
    let theme_index = ThemeMode::ALL.iter().position(|m| *m == prefs.theme).unwrap_or(0);
    let accent_index = Accent::ALL.iter().position(|a| *a == prefs.accent).unwrap_or(0);
    section("Theme", p).child(
        row()
            .child(div().w(px(260.)).child(theme_picker(theme_index)))
            .child(div().flex_1().min_w(px(420.)).child(accent_picker(accent_index))),
    )
}

/// Picks the app theme, saving the choice in the settings.
fn theme_picker(selected: usize) -> Segmented {
    Segmented::new("theme", "Theme", ThemeMode::ALL.map(ThemeMode::label), selected).on_select(
        |i, _, cx| {
            settings::update(cx, |s| s.prefs.theme = ThemeMode::ALL[*i]);
        },
    )
}

/// Picks the accent colour, saving the choice in the settings.
fn accent_picker(selected: usize) -> Segmented {
    Segmented::new("accent", "Accent", Accent::ALL.map(Accent::key), selected).on_select(
        |i, _, cx| {
            settings::update(cx, |s| s.prefs.accent = Accent::ALL[*i]);
        },
    )
}

/// Every button style and state, plus the two-tone icons.
pub(super) fn buttons(p: &Palette) -> Div {
    section("Buttons", p).child(text_buttons()).child(icon_buttons(p))
}

/// Text buttons in each style, and a disabled one.
fn text_buttons() -> Div {
    row()
        .child(
            Button::new("b-primary", "Create")
                .primary()
                .width(120.)
                .tooltip("Create playlist", Some("Enter")),
        )
        .child(Button::new("b-subtle", "Show Advanced Settings").icon(icons::SETTINGS))
        .child(
            Button::new("b-filled", "Show Advanced Settings")
                .icon(icons::SETTINGS)
                .style(ButtonStyle::Filled),
        )
        .child(Button::new("b-ghost", "Ghost").style(ButtonStyle::Ghost))
        .child(Button::new("b-disabled", "Disabled").disabled(true))
}

/// Icon buttons (plain, selected and disabled) and two-tone icons.
fn icon_buttons(p: &Palette) -> Div {
    row()
        .child(IconButton::new("i-play", icons::PLAY, "Play").shortcut("Space"))
        .child(IconButton::new("i-sub", icons::SUBTITLES, "Subtitles").shortcut("V").selected(true))
        .child(IconButton::new("i-vol", icons::VOLUME, "Mute").shortcut("M"))
        .child(IconButton::new("i-off", icons::SETTINGS, "Settings").disabled(true))
        .child(icon::two_tone(two_tone::CHECK_BOX, TWO_TONE_SIZE, p.icon, p.accent_fg))
        .child(icon::two_tone(two_tone::INFO, TWO_TONE_SIZE, p.icon, p.accent_fg))
}
