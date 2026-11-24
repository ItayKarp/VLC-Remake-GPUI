//! The bottom row of step 3 after saving failed (S0.2.3): the error where the hint
//! was, and Retry in place of Finish.

use gpui::{ClickEvent, Context, FontWeight, Hsla, SharedString, div, prelude::*, px};

use super::Onboarding;
use super::layout::{EDGE_BOTTOM, EDGE_LEFT, EDGE_RIGHT, NAV_BUTTON_WIDTH, back_button};
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::button::{Button, ButtonStyle};

/// Color of the error headline.
const DANGER: u32 = 0xE5484D;

/// Replaces step 3's bottom row when saving failed.
pub(super) fn render(
    error: SharedString,
    p: &Palette,
    cx: &mut Context<Onboarding>,
) -> impl IntoElement {
    div()
        .absolute()
        .left(px(EDGE_LEFT))
        .right(px(EDGE_RIGHT))
        .bottom(px(EDGE_BOTTOM))
        .flex()
        .flex_row()
        .items_end()
        .gap(px(24.))
        .child(message(error, p))
        .child(buttons(cx))
}

fn message(error: SharedString, p: &Palette) -> impl IntoElement {
    let danger: Hsla = gpui::rgb(DANGER).into();
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(
            div()
                .text_size(text_px(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(danger)
                .child("Your choices couldn't be saved."),
        )
        .child(
            div()
                .text_size(text_px(13.))
                .line_height(text_px(18.))
                .text_color(p.text_muted)
                .line_clamp(2)
                .child(error),
        )
}

fn buttons(cx: &mut Context<Onboarding>) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .gap(px(8.))
        .child(back_button(cx))
        .child(
            Button::new("continue-without-saving", "Continue without saving")
                .style(ButtonStyle::Filled)
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.continue_without_saving(window, cx);
                })),
        )
        .child(
            Button::new("retry", "Retry")
                .primary()
                .width(NAV_BUTTON_WIDTH)
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.finish(window, cx))),
        )
}
