//! Step 2: the theme tiles and accent swatches, around a live player preview.

use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, Hsla, KeyDownEvent, SharedString, Stateful,
    Window, div, prelude::*, px, svg,
};

use super::Onboarding;
use super::layout::{bottom_bar, radio_group, radio_step};
use super::preview;
use crate::assets::icons;
use crate::theme::{Accent, Palette, ThemeMode, blend};

const PREVIEW_INSET: f32 = 17.;
const PREVIEW_HEIGHT: f32 = 594.;
const SWATCH_SIZE: f32 = 40.;
const SWATCH_GAP: f32 = 5.;
const SWATCH_RING: f32 = 4.;
/// Distance between the centers of neighbouring accent swatches.
const SWATCH_PITCH: f32 = 80.;
const TILE_SIZE: f32 = 100.;
/// Tile fill for the light theme, and the light half of the system tile.
const TILE_LIGHT: u32 = 0xAFAFAF;
/// Tile fill for the dark theme.
const TILE_DARK: u32 = 0x212121;

impl Onboarding {
    /// Renders step 2: the preview with its theme picker, then swatches and nav buttons.
    pub(super) fn render_appearance(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        div()
            .relative()
            .size_full()
            .child(
                div()
                    .absolute()
                    .top(px(PREVIEW_INSET))
                    .left(px(PREVIEW_INSET))
                    .right(px(PREVIEW_INSET))
                    .h(px(PREVIEW_HEIGHT))
                    .child(self.render_preview(p, cx)),
            )
            .child(
                bottom_bar()
                    .left(px(43. - 6.))
                    .justify_between()
                    .child(self.render_swatches(p, cx))
                    .child(self.nav_buttons(true, "Next", cx)),
            )
            .into_any_element()
    }

    /// Moves the accent by `delta` places in [`Accent::ALL`], wrapping.
    pub(super) fn cycle_accent(&mut self, delta: isize, cx: &mut Context<Self>) {
        let len = Accent::ALL.len() as isize;
        let index = Accent::ALL.iter().position(|a| *a == self.accent).unwrap_or(0) as isize;
        self.accent = Accent::ALL[(index + delta).rem_euclid(len) as usize];
        cx.notify();
    }

    /// A miniature player window that reflects the chosen theme and accent.
    fn render_preview(&self, p: &Palette, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(p.sunken)
            .overflow_hidden()
            .child(preview::title_bar(p))
            .child(self.theme_picker(p, cx))
            .child(preview::control_bar(p))
    }

    /// The heading and theme tiles in the middle of the preview.
    fn theme_picker(&self, p: &Palette, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .pt(px(136.))
            .child(
                div()
                    .text_size(px(26.))
                    .line_height(px(34.))
                    .font_weight(FontWeight::LIGHT)
                    .text_color(p.text)
                    .child("Select your preferred theme and color."),
            )
            .child(
                // One tab stop; the arrow keys pick a theme (S0.4.14).
                radio_group("theme-tiles", "Theme", p)
                    .mt(px(66. - 6.))
                    .gap(px(34.))
                    .on_key_down(cx.listener(Self::on_theme_key))
                    .children(ThemeMode::ALL.map(|mode| self.theme_tile(mode, p, cx))),
            )
    }

    fn on_theme_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let current = ThemeMode::ALL.iter().position(|m| *m == self.theme);
        if let Some(next) = radio_step(event, current, ThemeMode::ALL.len()) {
            self.theme = ThemeMode::ALL[next];
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn theme_tile(&self, mode: ThemeMode, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let selected = self.theme == mode;
        div()
            .id(SharedString::from(format!("theme-{}", mode.key())))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(16.))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.theme = mode;
                cx.notify();
            }))
            .child(
                div()
                    .p(px(8.))
                    .rounded(px(22.))
                    .border(px(3.))
                    .border_color(if selected { p.accent } else { gpui::transparent_black() })
                    .child(theme_swatch(mode)),
            )
            .child(
                div()
                    .text_size(px(16.))
                    .text_color(if selected { p.text_strong } else { p.text })
                    .child(mode.label()),
            )
    }

    fn render_swatches(&self, p: &Palette, cx: &mut Context<Self>) -> impl IntoElement {
        radio_group("accent-swatches", "Accent color", p)
            .on_key_down(cx.listener(Self::on_accent_key))
            .gap(px(SWATCH_PITCH - SWATCH_SIZE - 2. * (SWATCH_GAP + SWATCH_RING)))
            .children(Accent::ALL.map(|accent| self.accent_swatch(accent, p, cx)))
    }

    fn on_accent_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let current = Accent::ALL.iter().position(|a| *a == self.accent);
        if let Some(next) = radio_step(event, current, Accent::ALL.len()) {
            self.accent = Accent::ALL[next];
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn accent_swatch(&self, accent: Accent, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let selected = self.accent == accent;
        let color = accent.color();
        let ring =
            if selected { blend(color, p.background, 0.3) } else { gpui::transparent_black() };
        let fill = if selected { color } else { blend(color, p.background, 0.5) };
        div()
            .id(SharedString::from(format!("accent-{}", accent.key())))
            .p(px(SWATCH_GAP))
            .rounded_full()
            .border(px(SWATCH_RING))
            .border_color(ring)
            .cursor_pointer()
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.accent = accent;
                cx.notify();
            }))
            .child(div().size(px(SWATCH_SIZE)).rounded_full().bg(fill))
    }
}

/// The colored square inside a theme tile; the system tile is half light, half dark.
fn theme_swatch(mode: ThemeMode) -> Div {
    let light: Hsla = gpui::rgb(TILE_LIGHT).into();
    let dark: Hsla = gpui::rgb(TILE_DARK).into();
    div()
        .relative()
        .size(px(TILE_SIZE))
        .rounded(px(12.))
        .overflow_hidden()
        .bg(if mode == ThemeMode::Light { light } else { dark })
        .when(mode == ThemeMode::System, |this| {
            this.child(
                svg()
                    .absolute()
                    .top_0()
                    .left_0()
                    .path(icons::THEME_SYSTEM)
                    .size(px(TILE_SIZE))
                    .text_color(light),
            )
        })
}
