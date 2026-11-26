//! Open Capture Device in the main window: the menu command (Ctrl+Shift+C) opens the dialog,
//! and its Play replaces the queue with the device and plays it.

use gpui::{App, AppContext as _, Context, Div, Window, prelude::*};

use super::CaptureDialog;
use crate::app::media::Media;
use crate::app::request::OpenRequest;
use crate::keymap::actions::OpenCaptureDevice;
use crate::main_window::MainWindow;

impl MainWindow {
    /// Adds the handler for Open Capture Device.
    pub(in crate::main_window) fn capture_actions(root: Div, cx: &mut Context<Self>) -> Div {
        root.on_action(cx.listener(|this, _: &OpenCaptureDevice, window, cx| {
            this.show_open_capture(window, cx);
        }))
    }

    /// Opens the Open Capture Device dialog.
    fn show_open_capture(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let main = cx.entity().downgrade();
        let on_play = move |item: Media, window: &mut Window, cx: &mut App| {
            let request = OpenRequest::replace(vec![item]);
            main.update(cx, |main, cx| main.open(request, window, cx)).ok();
        };
        let dialog = cx.new(|cx| CaptureDialog::new(on_play, window, cx));
        self.modals.update(cx, |layer, cx| layer.push(dialog, window, cx));
    }
}
