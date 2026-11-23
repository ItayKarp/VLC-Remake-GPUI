//! Open Disc in the main window: the menu command (Ctrl+D) opens the dialog, and its Play
//! replaces the queue with the disc (or an audio CD's tracks) and plays it.

use gpui::{App, AppContext as _, Context, Div, Window, prelude::*};

use super::OpenDiscDialog;
use crate::app::media::Media;
use crate::app::request::OpenRequest;
use crate::keymap::actions::OpenDisc;
use crate::main_window::MainWindow;

impl MainWindow {
    /// Adds the handlers for Open Network Stream, Open Disc and Open Capture Device.
    pub(in crate::main_window) fn open_dialog_actions(root: Div, cx: &mut Context<Self>) -> Div {
        Self::network_actions(Self::disc_actions(Self::capture_actions(root, cx), cx), cx)
    }

    /// Adds the handler for Open Disc.
    fn disc_actions(root: Div, cx: &mut Context<Self>) -> Div {
        root.on_action(
            cx.listener(|this, _: &OpenDisc, window, cx| this.show_open_disc(window, cx)),
        )
    }

    /// Opens the Open Disc dialog.
    fn show_open_disc(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let main = cx.entity().downgrade();
        let on_play = move |items: Vec<Media>, window: &mut Window, cx: &mut App| {
            main.update(cx, |main, cx| main.open(OpenRequest::replace(items), window, cx)).ok();
        };
        let dialog = cx.new(|cx| OpenDiscDialog::new(on_play, window, cx));
        self.modals.update(cx, |layer, cx| layer.push(dialog, window, cx));
    }
}
