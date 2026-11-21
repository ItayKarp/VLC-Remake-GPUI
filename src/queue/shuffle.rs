//! Shuffle (S3.3.2): the queue plays in a random order that repeats nothing until every entry
//! has played. With Repeat All a fresh order follows, and it never starts with the entry that
//! just played. Previous goes back through what played, and Next after it returns.
//!
//! The order is kept by entry id, beside the queue's own order, so moving or sorting rows
//! doesn't disturb it. Switching shuffle on keeps the current entry and shuffles the rest;
//! entries added meanwhile go to random places among those still to play.

use std::collections::{HashSet, VecDeque};

use super::random::Random;
use super::{EntryId, Queue};

/// How many played entries Previous can go back through.
const HISTORY_LIMIT: usize = 1_000;

/// The shuffled play order.
#[derive(Debug)]
pub(super) struct ShuffleOrder {
    /// The entries played before the current one, oldest first.
    past: VecDeque<EntryId>,
    /// The entries still to play this round; the next one is last.
    upcoming: Vec<EntryId>,
    random: Random,
}

impl ShuffleOrder {
    /// An order that plays `ids` at random.
    fn new(ids: Vec<EntryId>, random: Random) -> Self {
        let mut order = Self { past: VecDeque::new(), upcoming: Vec::new(), random };
        order.added(ids);
        order
    }

    /// The entry Next plays.
    pub fn next(&self) -> Option<EntryId> {
        self.upcoming.last().copied()
    }

    /// The entry Previous plays: the one that played before the current one.
    pub fn previous(&self) -> Option<EntryId> {
        self.past.back().copied()
    }

    /// `to` became current in place of `from`.
    pub fn moved(&mut self, from: Option<EntryId>, to: EntryId) {
        if from == Some(to) {
            return;
        }
        // Next first: a new round may start with the entry Previous would go back to.
        if self.next() != Some(to) && self.previous() == Some(to) {
            // Back through the history: what was current is next again.
            self.past.pop_back();
            self.upcoming.extend(from);
            return;
        }
        if let Some(from) = from {
            if self.past.len() == HISTORY_LIMIT {
                self.past.pop_front();
            }
            self.past.push_back(from);
        }
        if let Some(at) = self.upcoming.iter().rposition(|&id| id == to) {
            self.upcoming.remove(at);
        }
    }

    /// New entries go to random places among those still to play.
    pub fn added(&mut self, ids: impl IntoIterator<Item = EntryId>) {
        for id in ids {
            let at = self.random.up_to(self.upcoming.len());
            self.upcoming.insert(at, id);
        }
    }

    /// Entries left the queue: they won't play, and Previous skips them.
    pub fn removed(&mut self, ids: &HashSet<EntryId>) {
        self.past.retain(|id| !ids.contains(id));
        self.upcoming.retain(|id| !ids.contains(id));
    }

    /// Forgets everything, for a queue that was replaced.
    pub fn clear(&mut self) {
        self.past.clear();
        self.upcoming.clear();
    }

    /// Whether every entry of this round has played.
    pub fn round_is_over(&self) -> bool {
        self.upcoming.is_empty()
    }

    /// Starts a new round over `ids`, not starting with `current` unless it's the only one.
    pub fn new_round(&mut self, ids: impl Iterator<Item = EntryId>, current: Option<EntryId>) {
        let mut ids: Vec<EntryId> = ids.collect();
        self.random.shuffle(&mut ids);
        let last = ids.len().saturating_sub(1);
        if last > 0 && ids.last() == current.as_ref() {
            let other = self.random.up_to(last - 1);
            ids.swap(other, last);
        }
        self.upcoming = ids;
    }
}

impl Queue {
    /// Whether the queue plays in a random order.
    pub fn is_shuffled(&self) -> bool {
        self.shuffle.is_some()
    }

    /// Switches shuffle on or off. On, the current entry stays current and the rest play in a
    /// random order; off, Next goes on from the current entry in the queue's order.
    pub fn set_shuffle(&mut self, on: bool) {
        self.set_shuffle_with(on, Random::from_clock());
    }

    /// [`Self::set_shuffle`] with a given random sequence (for tests).
    pub(super) fn set_shuffle_with(&mut self, on: bool, random: Random) {
        if on == self.is_shuffled() {
            return;
        }
        if !on {
            self.shuffle = None;
            return;
        }
        let current = self.current_id();
        let rest = self.entries.iter().map(|entry| entry.id).filter(|&id| Some(id) != current);
        self.shuffle = Some(ShuffleOrder::new(rest.collect(), random));
        self.settle();
    }

    /// Under shuffle with Repeat All, starts a new round once every entry has played, so Next
    /// always has somewhere to go.
    pub(super) fn settle(&mut self) {
        let current = self.current_id();
        let Some(order) = &mut self.shuffle else { return };
        if self.repeat.wraps() && order.round_is_over() && !self.entries.is_empty() {
            order.new_round(self.entries.iter().map(|entry| entry.id), current);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::app::media::Media;
    use crate::queue::Repeat;

    fn shuffled(len: usize, current: usize, seed: u64) -> Queue {
        let mut queue = Queue::default();
        queue.replace((0..len).map(|i| Media::File(PathBuf::from(i.to_string()))).collect());
        queue.set_current(current);
        queue.set_shuffle_with(true, Random::seeded(seed));
        queue
    }

    /// Plays Next `count` times; returns the indices played.
    fn play_next(queue: &mut Queue, count: usize) -> Vec<usize> {
        (0..count)
            .map(|_| {
                let next = queue.next().expect("a next entry");
                queue.set_current(next);
                next
            })
            .collect()
    }

    fn distinct(indices: &[usize]) -> usize {
        indices.iter().collect::<HashSet<_>>().len()
    }

    #[test]
    fn plays_every_entry_once_then_stops() {
        let mut q = shuffled(10, 4, 1);
        assert_eq!(q.current_index(), Some(4), "the current entry stays current");
        let played = play_next(&mut q, 9);
        assert_eq!(distinct(&played), 9);
        assert!(!played.contains(&4));
        assert_eq!(q.next(), None);
    }

    #[test]
    fn repeat_all_starts_a_fresh_round_without_a_repeat_at_the_boundary() {
        for seed in 1..50 {
            let mut q = shuffled(5, 0, seed);
            q.set_repeat(Repeat::All);
            let first = play_next(&mut q, 4);
            let last = *first.last().expect("four played");
            let second = play_next(&mut q, 5);
            assert_eq!(distinct(&second), 5, "seed {seed}");
            assert_ne!(second[0], last, "seed {seed}");
        }
    }

    #[test]
    fn a_single_entry_repeats_under_repeat_all() {
        let mut q = shuffled(1, 0, 3);
        assert_eq!(q.next(), None);
        q.set_repeat(Repeat::All);
        assert_eq!(q.next(), Some(0));
    }

    #[test]
    fn previous_follows_the_history_and_next_comes_back() {
        let mut q = shuffled(8, 0, 9);
        let played = play_next(&mut q, 3);
        assert_eq!(q.previous(), Some(played[1]));
        q.set_current(played[1]);
        assert_eq!(q.previous(), Some(played[0]));
        assert_eq!(q.next(), Some(played[2]));
        q.set_current(played[2]);
        let rest = play_next(&mut q, 4);
        assert_eq!(distinct(&[played, rest].concat()), 7);
    }

    #[test]
    fn added_entries_play_in_this_round_and_removed_ones_never() {
        let mut q = shuffled(4, 0, 5);
        q.append((0..3).map(|i| Media::File(PathBuf::from(format!("new {i}")))).collect());
        let gone: HashSet<EntryId> = [q.entries()[1].id].into();
        q.remove(&gone);
        let played = play_next(&mut q, 5);
        assert_eq!(distinct(&played), 5);
        assert_eq!(q.next(), None);
    }

    #[test]
    fn switching_off_goes_on_in_queue_order() {
        let mut q = shuffled(6, 2, 11);
        q.set_shuffle(false);
        assert!(!q.is_shuffled());
        assert_eq!(q.next(), Some(3));
        assert_eq!(q.previous(), Some(1));
    }

    #[test]
    fn a_replaced_queue_shuffles_its_new_entries() {
        let mut q = shuffled(3, 0, 13);
        q.replace((0..6).map(|i| Media::File(PathBuf::from(i.to_string()))).collect());
        q.set_current(0);
        assert_eq!(q.previous(), None);
        assert_eq!(distinct(&play_next(&mut q, 5)), 5);
    }
}
