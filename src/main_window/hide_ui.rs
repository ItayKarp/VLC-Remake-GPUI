//! Hide UI mode (S2.8.2, `hide_ui/player_hide_ui.png`): only the rounded video viewport with its
//! 10 px margin stays. Ctrl+\ (Cmd+\ on macOS) and the video's right-click menu switch it; Escape
//! leaves it (`fullscreen.rs`). With no title bar left, dragging the video moves the window
//! (macOS and Linux: GPUI can't start a window move on Windows), and the OS still resizes it at
//! the edges. Modals and toasts still show.
//!
//! The video's right-click menu (`video_menu.rs`) holds Hide UI, or Show UI while hidden.

use gpui::{Context, Div, MouseButton, Window, div, prelude::*};

use super::MainWindow;
use crate::window_chrome;

impl MainWindow {
    /// Hide UI's key and menu item: switches the mode.
    pub(super) fn toggle_hide_ui(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_hide_ui(!self.auto_hide.hide_ui, cx);
        // Hidden controls give the keyboard back to the player.
        if self.auto_hide.hide_ui && !self.focus_handle.is_focused(window) {
            self.focus_handle.focus(window, cx);
        }
    }

    /// Turns Hide UI mode on or off.
    pub(super) fn set_hide_ui(&mut self, on: bool, cx: &mut Context<Self>) {
        self.auto_hide.hide_ui = on;
        cx.notify();
    }

    /// The video inside the content area: in Hide UI, out of fullscreen, dragging it moves the
    /// window. A right-click opens its menu; clicks play, pause and go fullscreen (S2.10.1).
    pub(super) fn video_area(&self, video: Div, window: &Window, cx: &mut Context<Self>) -> Div {
        let area = div()
            .id("video-area")
            .size_full()
            .capture_any_mouse_down(cx.listener(Self::arm_video_click))
            .on_click(cx.listener(Self::on_video_click))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_video_right_click))
            .child(self.auto_hide.video_area.element())
            .child(video);
        let moves_window = self.auto_hide.hide_ui && !window.is_fullscreen();
        div().size_full().child(if moves_window { window_chrome::drags_window(area) } else { area })
    }
}
