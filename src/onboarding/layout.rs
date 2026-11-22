//! Layout pieces shared by the onboarding steps: the bottom row, the Back / Next
//! buttons and the radio groups.

use gpui::{ClickEvent, Context, Div, KeyDownEvent, Stateful, accesskit, div, prelude::*, px};

use super::Onboarding;
use super::finish::FinishState;
use crate::theme::Palette;
use crate::ui::button::{Button, ButtonStyle};
use crate::ui::focus::{self, Axis, Layout};

/// Space between step 3's text and the left window edge.
pub(super) const EDGE_LEFT: f32 = 62.;
/// Space between the bottom-row buttons and the right window edge.
pub(super) const EDGE_RIGHT: f32 = 62.;
/// Space between the bottom-row buttons and the bottom window edge.
pub(super) const EDGE_BOTTOM: f32 = 56.;
/// Width of the Back, Next, Finish and Retry buttons.
pub(super) const NAV_BUTTON_WIDTH: f32 = 100.;

impl Onboarding {
    /// Back / Next (or Finish) buttons in the bottom-right corner.
    pub(super) fn nav_buttons(
        &self,
        show_back: bool,
        next_label: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .gap(px(8.))
            .when(self.rerun, |this| this.child(cancel_button(cx)))
            .when(show_back, |this| this.child(back_button(cx)))
            .child(
                Button::new("next", next_label)
                    .primary()
                    .width(NAV_BUTTON_WIDTH)
                    .disabled(self.finish == FinishState::Saving)
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, window, cx| this.go_next(window, cx)),
                    ),
            )
    }
}

/// Cancel, offered when setup is run again: returns to the main window with nothing changed.
fn cancel_button(cx: &mut Context<Onboarding>) -> Button {
    Button::new("cancel", "Cancel")
        .style(ButtonStyle::Filled)
        .width(NAV_BUTTON_WIDTH)
        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.cancel(window, cx)))
}

/// The Back button, which returns to the previous step.
pub(super) fn back_button(cx: &mut Context<Onboarding>) -> Button {
    Button::new("back", "Back")
        .style(ButtonStyle::Filled)
        .width(NAV_BUTTON_WIDTH)
        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.go_back(cx)))
}

/// A row of choices that is a single tab stop with a focus ring; arrow keys move
/// the choice (theme tiles, accent swatches).
pub(super) fn radio_group(id: &'static str, label: &'static str, p: &Palette) -> Stateful<Div> {
    let group = div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .p(px(4.))
        .rounded(px(12.))
        .border_2()
        .border_color(gpui::transparent_black())
        .tab_index(0)
        .role(accesskit::Role::RadioGroup)
        .aria_label(label);
    focus::ring(group, p)
}

/// The choice an arrow key moves to in a radio group of `len`, wrapping.
pub(super) fn radio_step(
    event: &KeyDownEvent,
    current: Option<usize>,
    len: usize,
) -> Option<usize> {
    let movement = focus::group_key(event, Axis::Horizontal)?;
    focus::step(current, len, movement, Layout::WRAPPING, |_| true)
}

/// Row pinned to the bottom of the content area, aligned with the nav buttons.
pub(super) fn bottom_bar() -> Div {
    div()
        .absolute()
        .left_0()
        .right(px(EDGE_RIGHT))
        .bottom(px(EDGE_BOTTOM))
        .h(px(32.))
        .flex()
        .flex_row()
        .items_center()
}
