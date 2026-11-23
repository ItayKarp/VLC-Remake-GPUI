//! Renaming a bookmark in place (S8.3.1): its name turns into a field holding it, selected. Enter
//! or leaving the field keeps what was typed (a blank name keeps the old one) and Escape cancels.
//!
//! The rename follows the bookmark by its place, not its row, so a bookmark added meanwhile
//! doesn't move it onto another.

use std::time::Duration;

use gpui::{Context, Div, Entity, Focusable as _, Subscription, Window, div, prelude::*, px};

use crate::db::bookmarks::Bookmark;
use crate::main_window::MainWindow;
use crate::theme::Palette;
use crate::ui::metrics;
use crate::ui::text_input::{self, Look, TextInput};

/// The field's height and side padding.
const FIELD_HEIGHT: f32 = 28.;
const FIELD_PADDING: f32 = 6.;
/// The longest name the field takes.
const MAX_NAME: usize = 200;

/// A bookmark being renamed.
pub(super) struct Rename {
    /// The bookmark's place.
    position: Duration,
    input: Entity<TextInput>,
    _events: Subscription,
}

impl MainWindow {
    /// Turns the name of the bookmark at `index` into a field and gives it the keyboard.
    pub(super) fn start_bookmark_rename(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(bookmark) = self.bookmark_list(cx).get(index).cloned() else { return };
        let input = cx.new(|cx| {
            TextInput::new(window, cx)
                .with_text(&bookmark.name)
                .with_all_selected()
                .with_look(Look::Bare)
                .with_max_len(MAX_NAME)
                .with_label("Bookmark name")
        });
        let events = cx.subscribe_in(&input, window, Self::on_rename_input);
        input.read(cx).focus_handle(cx).focus(window, cx);
        let rename = Rename { position: bookmark.position, input, _events: events };
        self.queue_panel.bookmarks.rename = Some(rename);
        cx.notify();
    }

    /// The field, if `bookmark` is the one being renamed.
    pub(super) fn renaming(&self, bookmark: &Bookmark) -> Option<&Entity<TextInput>> {
        let rename = self.queue_panel.bookmarks.rename.as_ref()?;
        (rename.position == bookmark.position).then_some(&rename.input)
    }

    fn on_rename_input(
        &mut self,
        _: &Entity<TextInput>,
        event: &text_input::Event,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            text_input::Event::Changed => {}
            text_input::Event::Submitted => {
                self.finish_bookmark_rename(cx);
                self.queue_panel.focus.focus(window, cx);
            }
            text_input::Event::Blurred => self.finish_bookmark_rename(cx),
        }
    }

    /// Keeps the typed name and puts the row back.
    fn finish_bookmark_rename(&mut self, cx: &mut Context<Self>) {
        let Some(rename) = self.queue_panel.bookmarks.rename.take() else { return };
        let name = rename.input.read(cx).text().to_owned();
        let index = self.bookmark_list(cx).iter().position(|b| b.position == rename.position);
        if let Some(index) = index {
            self.rename_bookmark(index, &name, cx);
        }
        cx.notify();
    }

    /// Puts the row back as it was and gives the keyboard back to the panel.
    pub(super) fn cancel_bookmark_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.queue_panel.bookmarks.rename.take().is_some() {
            self.queue_panel.focus.focus(window, cx);
            cx.notify();
        }
    }
}

/// The field in the name's place.
pub(super) fn field(input: &Entity<TextInput>, p: &Palette) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .h(px(FIELD_HEIGHT))
        .flex()
        .items_center()
        .px(px(FIELD_PADDING))
        .rounded(px(metrics::RADIUS))
        .bg(p.sunken)
        .border_1()
        .border_color(p.focus_ring)
        .text_color(p.text_strong)
        .child(input.clone())
}
