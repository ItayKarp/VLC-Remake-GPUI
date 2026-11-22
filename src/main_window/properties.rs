//! Properties & Metadata (S4.3.5; not designed): a read-only dialog of what the library read
//! from a file, its tags, length, resolution, codecs and cover source. It opens for the playing
//! file from the video menu and Cmd/Ctrl+I, and for a library file from its menu (S4.8.1). The
//! database is read on the history thread, so opening it never waits on the disk.

use std::path::{Path, PathBuf};

use futures::channel::oneshot;
use gpui::{AnyElement, App, Context, Div, SharedString, Window, div, prelude::*, px};

use super::MainWindow;
use crate::history::History;
use crate::keymap::actions::ShowProperties;
use crate::library::properties::{self, Properties};
use crate::theme;
use crate::ui::modal::{Dialog, DialogButton};

/// The width of the label column.
const LABEL_WIDTH: f32 = 110.;
/// The space between rows.
const ROW_GAP: f32 = 6.;

impl MainWindow {
    /// Adds Properties & Metadata's handler.
    pub(super) fn properties_actions(root: Div, cx: &mut Context<Self>) -> Div {
        root.on_action(
            cx.listener(|this, _: &ShowProperties, window, cx| this.show_properties(window, cx)),
        )
    }

    /// Whether Properties & Metadata has a file to show: one is loaded.
    pub(super) fn has_properties(&self, cx: &App) -> bool {
        self.loaded_player(cx).is_some_and(|player| {
            player.read(cx).media().is_some_and(|media| media.as_path().is_some())
        })
    }

    /// Reads the playing file's properties and opens the dialog with them.
    fn show_properties(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let player = self.loaded_player(cx);
        let path =
            player.and_then(|player| player.read(cx).media()?.as_path().map(Path::to_path_buf));
        if let Some(path) = path {
            Self::show_properties_of(path, window, cx);
        }
    }

    /// Reads the properties of the file at `path` and opens the dialog with them (the library's
    /// file menu, S4.8.1).
    pub(super) fn show_properties_of(path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let Some(history) = History::get(cx) else { return };
        let (reply, answer) = oneshot::channel();
        history.with_database(move |connection| {
            let read = properties::read(connection, &path).unwrap_or_else(|error| {
                tracing::warn!("library: can't read properties: {error}");
                Properties { path, ..Properties::default() }
            });
            let _ = reply.send(read);
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(read) = answer.await else { return };
            this.update_in(cx, |this, window, cx| this.open_properties(&read, window, cx)).ok();
        })
        .detach();
    }

    fn open_properties(&mut self, read: &Properties, window: &mut Window, cx: &mut Context<Self>) {
        let rows = properties::rows(read).into_iter().map(|(label, value)| (label, value.into()));
        self.show_info_dialog("Properties & Metadata", rows.collect(), window, cx);
    }

    /// Opens a read-only dialog titled `title` of `rows`: a muted label and its value on each
    /// (Properties & Metadata, and the library's folder Info and playlist properties).
    pub(super) fn show_info_dialog(
        &mut self,
        title: &'static str,
        rows: Vec<(&'static str, SharedString)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let body = move |window: &mut Window, cx: &mut App| body(&rows, window, cx);
        let buttons = vec![DialogButton::new("Close").primary()];
        let dialog = cx.new(|cx| Dialog::new(title, body, buttons, cx));
        self.modals.update(cx, |layer, cx| layer.push(dialog, window, cx));
    }
}

fn body(rows: &[(&'static str, SharedString)], window: &mut Window, cx: &mut App) -> AnyElement {
    let p = theme::palette(window, cx);
    let row = |(label, value): &(&'static str, SharedString)| {
        div()
            .flex()
            .gap(px(ROW_GAP * 2.))
            .child(div().w(px(LABEL_WIDTH)).flex_none().text_color(p.text_muted).child(*label))
            .child(div().flex_1().min_w_0().child(value.clone()))
    };
    div().flex().flex_col().gap(px(ROW_GAP)).children(rows.iter().map(row)).into_any_element()
}
