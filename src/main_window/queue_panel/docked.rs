//! The Queue tab docked in the miniplayer (S4.10.4, `miniplayer-variant2`): the same header and
//! rows as the List panel's, with their keys, drags and drops, above full library mode's card.
//! Escape closes it.

use gpui::{Context, Div, KeyDownEvent, Stateful, Window, div, prelude::*};

use super::MainWindow;
use crate::i18n::DirectionalStyle;
use crate::theme::Palette;
use crate::ui::focus;

impl MainWindow {
    /// The Queue tab for the miniplayer, filling what it is put in.
    pub(in crate::main_window) fn render_docked_queue(
        &mut self,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        self.watch_lengths(cx);
        self.sync_queue_panel(cx);
        div()
            .id("docked-queue")
            .flex_1()
            .min_h_0()
            .flex_row_start()
            .flex_col()
            .track_focus(&self.queue_panel.focus)
            .key_context(focus::context(&["QueuePanel", focus::CONTROL]))
            .on_key_down(cx.listener(Self::on_docked_queue_key))
            .child(self.render_queue_tab(p, cx))
    }

    /// Gives the docked queue the keyboard, once the click that opened it is done.
    pub(in crate::main_window) fn focus_docked_queue(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let handle = self.queue_panel.focus.clone();
        window.defer(cx, move |window, cx| handle.focus(window, cx));
    }

    /// Hands the keyboard back to the library's listing if the docked queue had it.
    pub(in crate::main_window) fn blur_docked_queue(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.queue_panel.sort_menu = None;
        if self.queue_panel.focus.contains_focused(window, cx) {
            self.focus_library_listing(window, cx);
        }
    }

    fn on_docked_queue_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key == "escape" {
            cx.stop_propagation();
            self.toggle_miniplayer_queue(window, cx);
        } else if self.queue_key(event, window, cx) {
            cx.stop_propagation();
        }
    }
}
