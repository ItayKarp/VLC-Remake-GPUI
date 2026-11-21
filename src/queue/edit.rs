//! Editing the queue from its panel (S3.2): removing entries, moving them and reordering the
//! whole list. Playback goes on through every edit: the current entry is followed by its id
//! wherever it moves, and when it is removed its place stays current (see [`Queue`]).

use std::collections::HashSet;

use super::{Current, Entry, EntryId, Queue};

impl Queue {
    /// The index of the entry `id`, if it is queued. O(n).
    pub fn index_of(&self, id: EntryId) -> Option<usize> {
        self.entries.iter().position(|entry| entry.id == id)
    }

    /// Removes the entries in `ids`; returns how many were queued. Removing the current one
    /// leaves its place current, so Next plays the first entry after it that stays.
    pub fn remove(&mut self, ids: &HashSet<EntryId>) -> usize {
        let before = self.entries.len();
        // How many kept entries come before each old index, so positions can be mapped.
        let kept_before = |index: usize, entries: &[Entry]| {
            entries[..index.min(entries.len())].iter().filter(|e| !ids.contains(&e.id)).count()
        };
        self.current = self.current.map(|current| match current {
            Current::At(at) if ids.contains(&self.entries[at].id) => {
                Current::Removed { next: kept_before(at, &self.entries) }
            }
            Current::At(at) => Current::At(kept_before(at, &self.entries)),
            Current::Removed { next } => {
                Current::Removed { next: kept_before(next, &self.entries) }
            }
        });
        self.entries.retain(|entry| !ids.contains(&entry.id));
        if let Some(order) = &mut self.shuffle {
            order.removed(ids);
        }
        let removed = before - self.entries.len();
        if removed > 0 {
            self.revision += 1;
            self.settle();
        }
        removed
    }

    /// Moves the entries in `ids`, keeping their order, to the gap `slot` (0 is before the
    /// first entry, [`Self::len`] after the last), as a drag and drop does.
    pub fn move_to(&mut self, ids: &HashSet<EntryId>, slot: usize) {
        // The first entry that stays put at or after the gap: the moved ones go before it.
        let before = self.entries.iter().skip(slot).find(|entry| !ids.contains(&entry.id));
        let before = before.map(|entry| entry.id);
        self.reorder(|entries| {
            let (moved, mut rest): (Vec<Entry>, Vec<Entry>) =
                entries.drain(..).partition(|entry| ids.contains(&entry.id));
            let at = before
                .and_then(|id| rest.iter().position(|entry| entry.id == id))
                .unwrap_or(rest.len());
            rest.splice(at..at, moved);
            *entries = rest;
        });
    }

    /// Reorders the entries with `arrange`, which must keep every entry (a sort or a shuffle).
    /// The current entry stays current wherever it lands.
    pub fn reorder(&mut self, arrange: impl FnOnce(&mut Vec<Entry>)) {
        let id_at = |index: usize| self.entries.get(index).map(|entry| entry.id);
        // Removed { next } past the end has no entry to follow: it stays at the end.
        let followed = self.current.map(|current| match current {
            Current::At(at) => (true, id_at(at)),
            Current::Removed { next } => (false, id_at(next)),
        });
        arrange(&mut self.entries);
        self.revision += 1;
        let Some((at, id)) = followed else { return };
        let index = id.and_then(|id| self.index_of(id)).unwrap_or(self.entries.len());
        self.current = Some(if at { Current::At(index) } else { Current::Removed { next: index } });
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::app::media::Media;

    fn queue(names: &[&str]) -> Queue {
        let mut queue = Queue::default();
        queue.replace(names.iter().map(|name| Media::File(PathBuf::from(name))).collect());
        queue
    }

    fn names(queue: &Queue) -> String {
        queue.entries().iter().map(|entry| entry.media.display_name()).collect()
    }

    fn ids(queue: &Queue, indices: &[usize]) -> HashSet<EntryId> {
        indices.iter().map(|&index| queue.entries()[index].id).collect()
    }

    #[test]
    fn removing_several_keeps_the_current_entry_current() {
        let mut q = queue(&["a", "b", "c", "d", "e"]);
        q.set_current(3);
        let gone = ids(&q, &[0, 2, 4]);
        assert_eq!(q.remove(&gone), 3);
        assert_eq!(names(&q), "bd");
        assert_eq!(q.current_index(), Some(1));
        assert_eq!(q.remove(&gone), 0);
    }

    #[test]
    fn removing_the_current_entry_plays_the_next_kept_one() {
        let mut q = queue(&["a", "b", "c", "d"]);
        q.set_current(1);
        let gone = ids(&q, &[1, 2]);
        q.remove(&gone);
        assert_eq!(q.current_index(), None);
        assert_eq!(q.next(), Some(1));
        assert_eq!(q.entries()[1].media.display_name(), "d");
        assert_eq!(q.previous(), Some(0));
    }

    #[test]
    fn removing_everything_while_playing_leaves_nothing_next() {
        let mut q = queue(&["a", "b"]);
        q.set_current(0);
        q.remove(&ids(&q, &[0, 1]));
        assert_eq!(q.len(), 0);
        assert!(q.has_current());
        q.set_repeat(crate::queue::Repeat::All);
        assert_eq!(q.next(), None);
    }

    #[test]
    fn moving_keeps_order_and_follows_the_current_entry() {
        let mut q = queue(&["a", "b", "c", "d", "e"]);
        q.set_current(1);
        // Drag b and d to the end.
        q.move_to(&ids(&q, &[1, 3]), 5);
        assert_eq!(names(&q), "acebd");
        assert_eq!(q.current_index(), Some(3));
        // And to the top.
        q.move_to(&ids(&q, &[3, 4]), 0);
        assert_eq!(names(&q), "bdace");
        assert_eq!(q.current_index(), Some(0));
    }

    #[test]
    fn moving_onto_itself_changes_nothing() {
        let mut q = queue(&["a", "b", "c"]);
        q.move_to(&ids(&q, &[1]), 1);
        assert_eq!(names(&q), "abc");
        q.move_to(&ids(&q, &[1]), 2);
        assert_eq!(names(&q), "abc");
        q.move_to(&ids(&q, &[0]), 2);
        assert_eq!(names(&q), "bac");
    }

    #[test]
    fn reordering_after_removing_the_current_entry_follows_its_successor() {
        let mut q = queue(&["a", "b", "c"]);
        q.set_current(0);
        q.remove(&ids(&q, &[0]));
        q.reorder(|entries| entries.reverse());
        assert_eq!(names(&q), "cb");
        assert_eq!(q.next(), Some(1));
    }

    #[test]
    fn inserting_before_a_removed_current_entry_keeps_its_successor_next() {
        let mut q = queue(&["a", "b", "c"]);
        q.set_current(1);
        q.remove(&ids(&q, &[1]));
        q.insert_at(0, vec![Media::File(PathBuf::from("x"))]);
        assert_eq!(names(&q), "xac");
        assert_eq!(q.next(), Some(2));
    }
}
