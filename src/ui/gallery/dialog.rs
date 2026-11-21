//! The gallery's demo dialogs: a form-like one, and a confirmation that stacks on top of it.

use gpui::{AnyElement, App, Context, Entity, WeakEntity, Window, div, prelude::*, px};

use super::Gallery;
use crate::ui::modal::{Dialog, DialogButton};
use crate::ui::toast::{Level, Toast};

impl Gallery {
    /// Opens the "New Playlist" dialog.
    pub(super) fn open_playlist_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let buttons = playlist_buttons(&cx.entity().downgrade());
        let dialog = cx.new(|cx| Dialog::new("New Playlist", playlist_body, buttons, cx));
        self.push_dialog(dialog, window, cx);
    }

    /// Opens a confirmation on top of the current dialog.
    fn open_confirm_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let buttons = vec![DialogButton::new("Cancel"), DialogButton::new("Yes").primary()];
        let dialog = cx.new(|cx| Dialog::new("Are you sure?", confirm_body, buttons, cx));
        self.push_dialog(dialog, window, cx);
    }

    fn push_dialog(&self, dialog: Entity<Dialog>, window: &mut Window, cx: &mut Context<Self>) {
        self.widgets.modals.update(cx, |layer, cx| layer.push(dialog, window, cx));
    }
}

fn playlist_body(_: &mut Window, _: &mut App) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child("Tab stays inside the dialog. Enter presses Create.")
        .into_any_element()
}

fn confirm_body(_: &mut Window, _: &mut App) -> AnyElement {
    div().child("This dialog is on top of another. Escape closes only this one.").into_any_element()
}

/// Cancel, a button that stacks a second dialog, and Create, which reports with a toast.
fn playlist_buttons(gallery: &WeakEntity<Gallery>) -> Vec<DialogButton> {
    let ask_again = gallery.clone();
    let create = gallery.clone();
    vec![
        DialogButton::new("Cancel"),
        DialogButton::new("Ask again…").on_click(move |window, cx| {
            ask_again.update(cx, |this, cx| this.open_confirm_dialog(window, cx)).ok();
        }),
        DialogButton::new("Create").primary().on_click(move |_, cx| {
            let toast = Toast::new(Level::Success, "Playlist created");
            create.update(cx, |this, cx| this.push_toast(toast, cx)).ok();
        }),
    ]
}
