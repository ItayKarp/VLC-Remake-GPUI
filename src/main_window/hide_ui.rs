//! Hide UI mode (S2.8.2, `hide_ui/player_hide_ui.png`): only the rounded video viewport with its
//! 10 px margin stays. Ctrl+\ (Cmd+\ on macOS) and the video's right-click menu switch it; Escape
//! leaves it (`fullscreen.rs`). With no title bar left, dragging the video moves the window
//! (macOS and Linux: GPUI can't start a window move on Windows), and the OS still resizes it at
//! the edges. Modals and toasts still show.
//!
//! The right-click menu holds only Hide UI (Show UI while hidden) until S2.9.2 fills it.

use gpui::{
    AnyElement, Context, DismissEvent, Div, Entity, MouseButton, MouseDownEvent, Pixels, Point,
    Subscription, Window, div, prelude::*,
};

use super::MainWindow;
use crate::keymap;
use crate::keymap::actions::HideInterface;
use crate::ui::menu::{ContextMenu, MenuItem};
use crate::ui::popover;
use crate::window_chrome;

/// The video's right-click menu and where it opened.
pub(super) type VideoMenu = (Entity<ContextMenu>, Subscription, Point<Pixels>);

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
    /// window. A right-click opens its menu.
    pub(super) fn video_area(&self, video: Div, window: &Window, cx: &mut Context<Self>) -> Div {
        let area = div()
            .id("video-area")
            .size_full()
            .on_mouse_down(MouseButton::Right, cx.listener(Self::open_video_menu))
            .child(video);
        let moves_window = self.auto_hide.hide_ui && !window.is_fullscreen();
        div().size_full().child(if moves_window { window_chrome::drags_window(area) } else { area })
    }

    fn open_video_menu(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_popovers(window, cx);
        self.menu = None;
        let label = if self.auto_hide.hide_ui { "Show UI" } else { "Hide UI" };
        let item = MenuItem::action(label, |window, cx| {
            window.dispatch_action(Box::new(HideInterface), cx);
        });
        let item = match keymap::shortcut(&HideInterface, cx) {
            Some(shortcut) => item.shortcut(shortcut),
            None => item,
        };
        let menu = ContextMenu::build(vec![item], window, cx);
        let subscription = cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
            this.auto_hide.video_menu = None;
            cx.notify();
        });
        self.auto_hide.video_menu = Some((menu, subscription, event.position));
        cx.notify();
    }

    /// The video's right-click menu, at the pointer.
    pub(super) fn render_video_menu(&self) -> Option<AnyElement> {
        let (menu, _, position) = self.auto_hide.video_menu.as_ref()?;
        Some(popover::at_position(*position, 1, menu.clone()))
    }
}
