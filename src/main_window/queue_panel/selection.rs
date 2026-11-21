//! The queue panel's multi-selection (S3.2.3): a click selects one row, Ctrl+click (Cmd+click
//! on macOS) adds or removes one, Shift+click selects the range from the last plain click, and
//! the arrow keys move it (Shift extends). Rows are named by entry id, so the selection follows
//! entries as they move.

use std::collections::HashSet;

use crate::queue::{Entry, EntryId};

/// Where the keyboard moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Movement {
    /// This many rows down (up when negative); from nothing, Down starts at the first row and
    /// Up at the last.
    By(isize),
    /// To the first row (Home).
    First,
    /// To the last row (End).
    Last,
}

/// The selected entries, the row ranges grow from, and the row the keyboard is on.
#[derive(Debug, Default)]
pub(super) struct Selection {
    ids: HashSet<EntryId>,
    /// Where Shift+click and Shift+arrow ranges start.
    anchor: Option<EntryId>,
    /// The row the arrow keys move from: the last one clicked or reached.
    cursor: Option<EntryId>,
}

impl Selection {
    /// The selected entries.
    pub(super) fn ids(&self) -> &HashSet<EntryId> {
        &self.ids
    }

    /// Whether `id` is selected.
    pub(super) fn contains(&self, id: EntryId) -> bool {
        self.ids.contains(&id)
    }

    /// Whether nothing is selected.
    pub(super) fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// The row the keyboard is on.
    pub(super) fn cursor(&self) -> Option<EntryId> {
        self.cursor
    }

    /// Selects only `id` (a plain click).
    pub(super) fn select(&mut self, id: EntryId) {
        self.ids.clear();
        self.ids.insert(id);
        self.anchor = Some(id);
        self.cursor = Some(id);
    }

    /// Adds `id` to the selection, or takes it out (Ctrl/Cmd+click).
    pub(super) fn toggle(&mut self, id: EntryId) {
        if !self.ids.remove(&id) {
            self.ids.insert(id);
        }
        self.anchor = Some(id);
        self.cursor = Some(id);
    }

    /// Selects the rows from the anchor to `id` (Shift+click).
    pub(super) fn extend_to(&mut self, entries: &[Entry], id: EntryId) {
        let position = |id: EntryId| entries.iter().position(|entry| entry.id == id);
        let Some(end) = position(id) else { return };
        let start = self.anchor.and_then(position).unwrap_or(end);
        let (low, high) = (start.min(end), start.max(end));
        self.ids = entries[low..=high].iter().map(|entry| entry.id).collect();
        self.anchor = Some(entries[start].id);
        self.cursor = Some(id);
    }

    /// Selects every row (Ctrl/Cmd+A).
    pub(super) fn select_all(&mut self, entries: &[Entry]) {
        self.ids = entries.iter().map(|entry| entry.id).collect();
    }

    /// Moves the keyboard as `movement` says (clamped to the list), selecting the row it lands
    /// on or, with `extend`, the range from the anchor to it. Returns the row's index.
    pub(super) fn step(
        &mut self,
        entries: &[Entry],
        movement: Movement,
        extend: bool,
    ) -> Option<usize> {
        let last = entries.len().checked_sub(1)?;
        let from = self.cursor.and_then(|id| entries.iter().position(|entry| entry.id == id));
        let to = match (movement, from) {
            (Movement::By(delta), Some(from)) => from.saturating_add_signed(delta).min(last),
            (Movement::Last, _) => last,
            (Movement::By(delta), None) if delta < 0 => last,
            (Movement::First | Movement::By(_), _) => 0,
        };
        let id = entries[to].id;
        if extend {
            self.extend_to(entries, id);
        } else {
            self.select(id);
        }
        Some(to)
    }

    /// Forgets entries that are no longer queued.
    pub(super) fn retain(&mut self, entries: &[Entry]) {
        let queued: HashSet<EntryId> = entries.iter().map(|entry| entry.id).collect();
        self.ids.retain(|id| queued.contains(id));
        self.anchor = self.anchor.filter(|id| queued.contains(id));
        self.cursor = self.cursor.filter(|id| queued.contains(id));
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::app::media::Media;
    use crate::queue::Queue;

    fn queue(len: usize) -> Queue {
        let mut queue = Queue::default();
        queue.replace((0..len).map(|i| Media::File(PathBuf::from(i.to_string()))).collect());
        queue
    }

    fn selected(selection: &Selection, entries: &[Entry]) -> Vec<usize> {
        let rows = entries.iter().enumerate();
        rows.filter(|(_, entry)| selection.contains(entry.id)).map(|(index, _)| index).collect()
    }

    #[test]
    fn clicks_toggles_and_ranges() {
        let q = queue(6);
        let e = q.entries();
        let mut s = Selection::default();
        s.select(e[1].id);
        s.toggle(e[4].id);
        assert_eq!(selected(&s, e), [1, 4]);
        // The range grows from the last clicked row.
        s.extend_to(e, e[2].id);
        assert_eq!(selected(&s, e), [2, 3, 4]);
        s.toggle(e[3].id);
        assert_eq!(selected(&s, e), [2, 4]);
        s.select_all(e);
        assert_eq!(selected(&s, e).len(), 6);
    }

    #[test]
    fn arrows_move_and_extend_within_the_list() {
        let q = queue(4);
        let e = q.entries();
        let mut s = Selection::default();
        assert_eq!(s.step(e, Movement::By(1), false), Some(0));
        assert_eq!(s.step(e, Movement::By(1), true), Some(1));
        assert_eq!(s.step(e, Movement::By(1), true), Some(2));
        assert_eq!(selected(&s, e), [0, 1, 2]);
        assert_eq!(s.step(e, Movement::By(99), false), Some(3));
        assert_eq!(selected(&s, e), [3]);
        assert_eq!(s.step(e, Movement::By(-99), false), Some(0));
        assert_eq!(s.step(e, Movement::Last, true), Some(3));
        assert_eq!(selected(&s, e), [0, 1, 2, 3]);
        assert_eq!(Selection::default().step(&[], Movement::First, false), None);
        let mut fresh = Selection::default();
        assert_eq!(fresh.step(e, Movement::First, false), Some(0));
        assert_eq!(fresh.step(e, Movement::By(-1), false), Some(0));
        assert_eq!(Selection::default().step(e, Movement::By(-1), false), Some(3));
    }

    #[test]
    fn removed_entries_leave_the_selection() {
        let mut q = queue(3);
        let mut s = Selection::default();
        s.select_all(q.entries());
        let gone: HashSet<EntryId> = [q.entries()[0].id].into();
        q.remove(&gone);
        s.retain(q.entries());
        assert_eq!(s.ids().len(), 2);
        assert!(!s.is_empty());
    }
}
