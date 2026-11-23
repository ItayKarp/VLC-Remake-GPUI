//! Keyboard handling for [`ContextMenu`]: activation, submenus, moving the
//! highlight and type-to-select.

use std::time::Instant;

use gpui::{Context, KeyDownEvent, Window};

use super::ContextMenu;
use crate::ui::dropdown::find_match;
use crate::ui::focus::{self, Axis};

impl ContextMenu {
    /// Handles a key pressed while this menu has focus.
    pub(super) fn on_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let modifiers = event.keystroke.modifiers;
        if modifiers.control || modifiers.platform || modifiers.alt {
            return;
        }
        if self.handle_key(event, window, cx) {
            cx.stop_propagation();
        }
    }

    /// Acts on a key. Returns whether the key was consumed.
    fn handle_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match event.keystroke.key.as_str() {
            "escape" => self.dismiss(window, cx),
            "enter" | "space" => {
                if let Some(index) = self.highlighted {
                    self.activate(index, window, cx);
                }
            }
            "right" => {
                if let Some(index) = self.highlighted
                    && self.items[index].is_submenu()
                {
                    self.open_submenu(index, true, window, cx);
                }
            }
            _ => return self.navigate_or_type(event, cx),
        }
        true
    }

    /// Moves the highlight or jumps to a typed item. Returns whether the key was
    /// consumed.
    fn navigate_or_type(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        if let Some(movement) = focus::group_key(event, Axis::Vertical) {
            self.move_highlight(movement, cx);
            return true;
        }
        let Some(ch) = event.keystroke.key_char.as_deref() else {
            return false;
        };
        if ch.trim().is_empty() {
            return false;
        }
        self.type_to_highlight(ch, cx);
        true
    }

    /// Highlights the next selectable item whose label starts with what was typed.
    fn type_to_highlight(&mut self, ch: &str, cx: &mut Context<Self>) {
        let items = &self.items;
        let labels: Vec<&str> =
            items.iter().map(|item| item.label().map_or("", AsRef::as_ref)).collect();
        let query = self.type_ahead.push(ch, Instant::now());
        let Some(index) = find_match(&labels, &query, self.highlighted, |i| items[i].selectable())
        else {
            return;
        };
        self.highlighted = Some(index);
        self.scroll.scroll_to_item(index);
        cx.notify();
    }

    /// Left or Escape in a submenu closes just that submenu. Runs in the capture
    /// phase, before the submenu sees the key.
    pub(super) fn on_key_down_capture_left(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event.keystroke.key.as_str(), "left" | "escape")
            && let Some(open) = &self.submenu
            && open.menu.read(cx).focus_handle.is_focused(window)
        {
            self.close_submenu(window, cx);
            cx.stop_propagation();
        }
    }
}
