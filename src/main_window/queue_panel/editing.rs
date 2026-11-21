//! What the Queue tab does to the queue (S3.2.3–S3.2.5): selecting and playing rows, removing
//! the selected ones, moving dragged rows, inserting dropped files and sorting. Playback goes on
//! through all of it; removing every row while something plays lets it finish.

use std::collections::HashSet;

use gpui::{ClickEvent, Context, ExternalPaths, KeyDownEvent, ScrollStrategy, Window};

use super::MainWindow;
use super::selection::Movement;
use crate::app::media::Media;
use crate::main_window::opening::Target;
use crate::queue::{EntryId, SortKey};

/// How many rows PageUp and PageDown move.
const PAGE: isize = 10;

impl MainWindow {
    /// A click on the row `id`: selects it (Ctrl/Cmd adds it, Shift selects the range to it), and
    /// a double-click plays it.
    pub(super) fn click_row(
        &mut self,
        id: EntryId,
        event: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.queue_panel.focus.focus(window, cx);
        let modifiers = event.modifiers();
        let selection = &mut self.queue_panel.selection;
        if modifiers.shift {
            selection.extend_to(self.queue.entries(), id);
        } else if modifiers.secondary(cx) {
            selection.toggle(id);
        } else {
            selection.select(id);
        }
        if event.click_count() >= 2 {
            self.play_entry(id, window, cx);
        }
        cx.notify();
    }

    /// The Queue tab's keys: the arrows, Home, End, PageUp and PageDown move the selection
    /// (Shift extends it), Enter plays, Delete and Backspace remove, Ctrl/Cmd+A selects all.
    /// Returns whether the key was one of them.
    pub(super) fn queue_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let keystroke = &event.keystroke;
        let (shift, secondary) = (keystroke.modifiers.shift, keystroke.modifiers.secondary(cx));
        let movement = match keystroke.key.as_str() {
            "up" => Movement::By(-1),
            "down" => Movement::By(1),
            "pageup" => Movement::By(-PAGE),
            "pagedown" => Movement::By(PAGE),
            "home" => Movement::First,
            "end" => Movement::Last,
            "enter" => return self.play_cursor(window, cx),
            "delete" | "backspace" => {
                self.remove_selected(window, cx);
                return true;
            }
            "a" if secondary => {
                self.queue_panel.selection.select_all(self.queue.entries());
                cx.notify();
                return true;
            }
            _ => return false,
        };
        if let Some(row) = self.queue_panel.selection.step(self.queue.entries(), movement, shift) {
            self.queue_panel.scroll.scroll_to_item(row, ScrollStrategy::Nearest);
            cx.notify();
        }
        true
    }

    /// Plays the row the keyboard is on.
    fn play_cursor(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(id) = self.queue_panel.selection.cursor() else { return false };
        self.play_entry(id, window, cx);
        true
    }

    /// Plays the entry `id`, as if the user had just started the queue there.
    fn play_entry(&mut self, id: EntryId, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.queue.index_of(id) {
            self.skips.played();
            self.play(index, window, cx);
        }
    }

    /// Removes the selected rows ("−", Delete), then selects the row that took the first one's
    /// place so the keyboard can carry on.
    pub(super) fn remove_selected(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let ids = self.queue_panel.selection.ids().clone();
        let first = self.queue.entries().iter().position(|entry| ids.contains(&entry.id));
        if self.queue.remove(&ids) == 0 {
            return;
        }
        let entries = self.queue.entries();
        self.queue_panel.selection.retain(entries);
        let next = first.and_then(|first| entries.get(first).or_else(|| entries.last()));
        if let Some(next) = next {
            self.queue_panel.selection.select(next.id);
        }
        cx.notify();
    }

    /// Removes every entry (the video menu's Clear Queue); what plays carries on to its end.
    pub(in crate::main_window) fn clear_queue(&mut self, cx: &mut Context<Self>) {
        let ids = self.queue.entries().iter().map(|entry| entry.id).collect();
        self.queue.remove(&ids);
        self.queue_panel.selection.retain(self.queue.entries());
        cx.notify();
    }

    /// Rows `ids` were dropped on the rows: they move to the gap under the pointer.
    pub(super) fn drop_rows(
        &mut self,
        ids: &HashSet<EntryId>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(slot) = self.queue_panel.drop_slot.take() {
            self.queue.move_to(ids, slot);
        }
        cx.notify();
    }

    /// Files from the OS were dropped on the rows: they're resolved like any open (folders
    /// scanned, playlists expanded) and inserted at the gap under the pointer.
    pub(super) fn drop_files(
        &mut self,
        paths: &ExternalPaths,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let slot = self.queue_panel.drop_slot.take().unwrap_or(self.queue.len());
        // The entry the files go before, so entries added meanwhile don't shift them.
        let before = self.queue.entries().get(slot).map(|entry| entry.id);
        let items = paths.paths().iter().cloned().map(Media::from).collect();
        self.resolve_into(items, Target::Before(before), window, cx);
    }

    /// Sorts the queue once by `key` (S3.2.5).
    pub(super) fn sort_queue(&mut self, key: SortKey, cx: &mut Context<Self>) {
        let lengths = &self.queue_panel.lengths;
        self.queue.sort(key, |media| lengths.duration(media));
        self.queue_panel.sort_menu = None;
        cx.notify();
    }
}
