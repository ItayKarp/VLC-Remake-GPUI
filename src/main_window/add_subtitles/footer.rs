//! The Add Subtitles dialog's footer: Cancel at the left, then "Copy next to video" (S6.4.6, not
//! in the design: linking is the default, D16) and what Save is doing or what went wrong, and
//! Save at the right.

use gpui::{ClickEvent, Context, Div, div, prelude::*, px};

use super::render::PADDING;
use super::{AddSubtitlesDialog, Status};
use crate::theme::Palette;
use crate::ui::button::{Button, ButtonStyle};
use crate::ui::metrics;
use crate::ui::toggle::Checkbox;

/// Cancel's and Save's width.
const BUTTON_WIDTH: f32 = 100.;

impl AddSubtitlesDialog {
    /// Cancel, the copy checkbox with the status, and Save.
    pub(super) fn footer(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let can_save = self.can_save(cx);
        let cancel = Button::new("add-subtitles-cancel", "Cancel")
            .style(ButtonStyle::Filled)
            .width(BUTTON_WIDTH)
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.cancel(cx)));
        let save = Button::new("add-subtitles-save", "Save")
            .style(if can_save { ButtonStyle::Primary } else { ButtonStyle::Ghost })
            .width(BUTTON_WIDTH)
            .disabled(!can_save)
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.save(window, cx)));
        let this = cx.entity().downgrade();
        let copy = Checkbox::new("add-subtitles-copy", self.copying.on, "Copy next to video")
            .disabled(self.saving.is_some())
            .on_change(move |on, _, cx| {
                this.update(cx, |dialog, cx| dialog.set_copying(*on, cx)).ok();
            });
        let status = self.status.as_ref().map(|status| status_line(status, p));
        let middle = div()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .gap(px(16.))
            .px(px(16.))
            .child(div().flex_none().child(copy))
            .children(status);
        div()
            .flex_none()
            .flex()
            .items_center()
            .p(px(PADDING))
            .child(cancel)
            .child(middle)
            .child(save)
    }

    /// The checkbox: copies are made on Save, their names checked now.
    fn set_copying(&mut self, on: bool, cx: &mut Context<Self>) {
        self.copying.on = on;
        self.check_conflicts(cx);
        cx.notify();
    }
}

/// What Save is doing, or what went wrong, in the danger color then.
fn status_line(status: &Status, p: &Palette) -> Div {
    div()
        .min_w_0()
        .truncate()
        .text_size(px(metrics::TEXT_SMALL))
        .text_color(if status.failed { p.danger } else { p.text_muted })
        .child(status.text.clone())
}
