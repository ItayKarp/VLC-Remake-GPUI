//! The play queue (S3.1): the ordered items the player works through, which of them is
//! current, and the order they played in.
//!
//! Every item is an [`Entry`] with an id of its own, so the same file queued twice is two
//! independent entries. Looking an item up, making it current and stepping to the next or
//! previous one are O(1); inserting and removing shift the items after them, which is a quick
//! memory move even for a 10,000-item queue.
//!
//! The queue panel (S3.2) edits it: removing, moving and sorting entries (`edit.rs`). Repeat
//! ([`Repeat`]) and Shuffle (`shuffle.rs`, S3.3) live here too, so the queue's buttons, the
//! menus and Settings → Playback all change this one state.
//!
//! When the current item is removed (the queue panel's Delete, S3.2), playback goes on and the
//! item stays in the history: VLC stops instead, but cutting off what the user is watching
//! because they tidied the list is a surprise. Next then plays the item that took its place.
//!
//! Nothing here touches GPUI: the main window owns a [`Queue`] and plays what it says.

mod edit;
mod lengths;
mod order;
mod random;
mod repeat;
mod shuffle;
mod skips;

use crate::app::media::Media;

use self::shuffle::ShuffleOrder;

pub use lengths::{Length, Lengths, row_duration};
pub use order::SortKey;
pub use repeat::Repeat;
pub use skips::{AfterError, MAX_CONSECUTIVE_FAILURES, Skips};

/// Names one entry for as long as it is queued, wherever it moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EntryId(u64);

/// One queued item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Tells this entry apart from others with the same media.
    pub id: EntryId,
    /// What it plays.
    pub media: Media,
}

/// Where the queue stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Current {
    /// The entry at this index is current.
    At(usize),
    /// The current entry was removed; the one now at `next` took its place.
    Removed { next: usize },
}

/// The play queue.
#[derive(Debug, Default)]
pub struct Queue {
    entries: Vec<Entry>,
    current: Option<Current>,
    /// What happens at an item's end and at the queue's ends (S3.3.1).
    repeat: Repeat,
    /// The random play order, while Shuffle is on (S3.3.2).
    shuffle: Option<ShuffleOrder>,
    next_id: u64,
    /// Changes whenever the entries do, so views can tell when to look again.
    revision: u64,
}

impl Queue {
    /// The queued entries, in play order.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// How many entries are queued.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// A number that changes whenever entries are added, removed or moved.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The index of the current entry; `None` when nothing is current or it was removed.
    pub fn current_index(&self) -> Option<usize> {
        match self.current? {
            Current::At(index) => Some(index),
            Current::Removed { .. } => None,
        }
    }

    /// Whether something was made current, even if it has since been removed.
    pub fn has_current(&self) -> bool {
        self.current.is_some()
    }

    /// The repeat mode.
    pub fn repeat(&self) -> Repeat {
        self.repeat
    }

    /// Changes the repeat mode.
    pub fn set_repeat(&mut self, repeat: Repeat) {
        self.repeat = repeat;
        self.settle();
    }

    /// Replaces everything with `media`; nothing is current until [`Self::set_current`].
    pub fn replace(&mut self, media: Vec<Media>) {
        self.entries.clear();
        self.current = None;
        if let Some(order) = &mut self.shuffle {
            order.clear();
        }
        self.revision += 1;
        self.append(media);
    }

    /// Adds `media` at the end. Returns the index of the first added entry.
    pub fn append(&mut self, media: Vec<Media>) -> usize {
        let first = self.entries.len();
        self.insert_at(first, media)
    }

    /// Adds `media` right after the current entry (Play Next), or at the end when nothing is
    /// current. Returns the index of the first added entry.
    pub fn insert_next(&mut self, media: Vec<Media>) -> usize {
        let at = match self.current {
            Some(Current::At(index)) => index + 1,
            Some(Current::Removed { next }) => next,
            None => self.entries.len(),
        };
        self.insert_at(at, media)
    }

    /// Makes the entry at `index` current and returns what it plays; `None` (and no change)
    /// when there's no such entry.
    pub fn set_current(&mut self, index: usize) -> Option<&Media> {
        let to = self.entries.get(index)?.id;
        let from = self.current_id();
        if let Some(order) = &mut self.shuffle {
            order.moved(from, to);
        }
        self.current = Some(Current::At(index));
        self.settle();
        Some(&self.entries[index].media)
    }

    /// The entry Next plays: the next one in play order, or `None` at the end of the queue
    /// (Repeat All wraps around; Repeat One doesn't hold Next back).
    pub fn next(&self) -> Option<usize> {
        let current = self.current?;
        if let Some(order) = &self.shuffle {
            return order.next().and_then(|id| self.index_of(id));
        }
        let next = match current {
            Current::At(index) => index + 1,
            Current::Removed { next } => next,
        };
        match next {
            next if next < self.len() => Some(next),
            _ if self.repeat.wraps() && !self.entries.is_empty() => Some(0),
            _ => None,
        }
    }

    /// The entry Previous plays: the one before the current one in play order (under shuffle,
    /// the one that played before it), or `None` at the start (Repeat All wraps around).
    pub fn previous(&self) -> Option<usize> {
        let current = self.current?;
        if let Some(order) = &self.shuffle {
            return order.previous().and_then(|id| self.index_of(id));
        }
        let before = match current {
            Current::At(index) | Current::Removed { next: index } => index.checked_sub(1),
        };
        match before {
            Some(previous) if previous < self.len() => Some(previous),
            _ if self.repeat.wraps() && !self.entries.is_empty() => Some(self.len() - 1),
            _ => None,
        }
    }

    /// The entry to play when the current one reaches its end: itself again under Repeat One,
    /// else what Next plays.
    pub fn after_end(&self) -> Option<usize> {
        match self.current_index() {
            Some(current) if self.repeat == Repeat::One => Some(current),
            _ => self.next(),
        }
    }

    /// The id of the current entry, unless it was removed.
    fn current_id(&self) -> Option<EntryId> {
        self.current_index().map(|index| self.entries[index].id)
    }

    /// Adds `media` before the entry at `at` (at the end past the last one). Returns where the
    /// first added entry went.
    pub fn insert_at(&mut self, at: usize, media: Vec<Media>) -> usize {
        let at = at.min(self.entries.len());
        let count = media.len();
        let entries: Vec<Entry> = media
            .into_iter()
            .map(|media| {
                self.next_id += 1;
                Entry { id: EntryId(self.next_id), media }
            })
            .collect();
        if let Some(order) = &mut self.shuffle {
            order.added(entries.iter().map(|entry| entry.id));
        }
        self.entries.splice(at..at, entries);
        self.revision += 1;
        match &mut self.current {
            Some(Current::At(index)) if *index >= at => *index += count,
            Some(Current::Removed { next }) if *next > at => *next += count,
            _ => {}
        }
        at
    }
}

#[cfg(test)]
mod tests;
