//! The queue above the miniplayer card (S4.10.4, `miniplayer-variant2`, `-variant4`): a "Queue"
//! title over the List panel's Queue tab, "N items - total" with sort, "−" and "+", and the
//! virtualized rows with their drag handles and lengths.

use gpui::{Context, Div, div, prelude::*, px};

use crate::main_window::MainWindow;
use crate::theme::Palette;
use crate::theme::text_px;

/// The title's height and text size.
const TITLE_HEIGHT: f32 = 44.;
const TITLE_TEXT: f32 = 15.;

impl MainWindow {
    /// The queue, `height` tall.
    pub(super) fn render_miniplayer_queue(
        &mut self,
        height: f32,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        let title = div()
            .flex_none()
            .h(px(TITLE_HEIGHT))
            .flex()
            .items_center()
            .justify_center()
            .bg(p.well)
            .text_size(text_px(TITLE_TEXT))
            .text_color(p.text)
            .child("Queue");
        div()
            .flex_none()
            .h(px(height))
            .flex()
            .flex_col()
            .child(title)
            .child(self.render_docked_queue(p, cx))
    }
}
