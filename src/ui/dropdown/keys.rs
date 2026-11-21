//! Keyboard handling for [`Dropdown`]: opening, closing, moving the highlight and
//! type-to-select.

use std::time::Instant;

use gpui::{Context, KeyDownEvent, Window};

use super::{Dropdown, VISIBLE_ROWS, find_match};
use crate::ui::focus::{self, Axis, Layout, Move};

impl Dropdown {
    /// Handles a key pressed while the dropdown has focus.
    pub(super) fn on_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.disabled && self.handle_key(event, window, cx) {
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
        let keystroke = &event.keystroke;
        let key = keystroke.key.as_str();
        if keystroke.modifiers.alt && matches!(key, "down" | "up") {
            self.toggle(window, cx);
            return true;
        }
        match (self.open, key) {
            (false, "enter" | "space" | "down") => self.open(window, cx),
            (true, "escape") => self.close(cx),
            (true, "enter" | "space") => {
                if let Some(index) = self.highlighted {
                    self.choose(index, cx);
                }
            }
            (true, "tab") => {
                // Close, and let Tab move focus on.
                self.close(cx);
                return false;
            }
            _ => return self.navigate_or_type(event, cx),
        }
        true
    }

    /// Moves the highlight (or the value, while closed) or selects by typing.
    /// Returns whether the key was consumed.
    fn navigate_or_type(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        if let Some(movement) = focus::group_key(event, Axis::Vertical) {
            self.move_by(movement, cx);
            return true;
        }
        let keystroke = &event.keystroke;
        let modifiers = keystroke.modifiers;
        let Some(ch) = keystroke.key_char.as_deref() else {
            return false;
        };
        if ch.trim().is_empty() || modifiers.control || modifiers.platform {
            return false;
        }
        self.type_to_select(ch, cx);
        true
    }

    fn move_by(&mut self, movement: Move, cx: &mut Context<Self>) {
        let layout = Layout { page: VISIBLE_ROWS, ..Layout::LIST };
        let len = self.options.len();
        if self.open {
            self.highlighted = focus::step(self.highlighted, len, movement, layout, |_| true);
            if let Some(index) = self.highlighted {
                self.scroll.scroll_to_item(index);
            }
            cx.notify();
            return;
        }
        // Up/Home/End while closed change the value, like a native select.
        if movement != Move::Next
            && let Some(index) = focus::step(Some(self.selected), len, movement, layout, |_| true)
        {
            self.select_closed(index, cx);
        }
    }

    fn type_to_select(&mut self, ch: &str, cx: &mut Context<Self>) {
        let query = self.type_ahead.push(ch, Instant::now());
        let labels: Vec<&str> = self.options.iter().map(AsRef::as_ref).collect();
        let current = if self.open { self.highlighted } else { Some(self.selected) };
        let Some(index) = find_match(&labels, &query, current, |_| true) else {
            return;
        };
        if self.open {
            self.highlighted = Some(index);
            self.scroll.scroll_to_item(index);
        } else {
            self.select_closed(index, cx);
        }
        cx.notify();
    }
}
