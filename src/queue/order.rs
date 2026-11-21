//! Sorting the queue once (S3.2.5): by name, by duration, by when entries were added, or into a
//! random order. It's a one-time reorder, not a sort mode: entries added later go where they're
//! put.

use std::cmp::Ordering;
use std::time::Duration;

use super::random::Random;
use super::{Entry, Queue};
use crate::app::media::Media;
use crate::opening::scan::natural_cmp;

/// How to sort the queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    /// By name, the way people read numbers (`Episode 2` before `Episode 10`).
    Name,
    /// Shortest first; entries whose length isn't known go last.
    Duration,
    /// In the order they were added.
    DateAdded,
    /// A random order, once (not the Shuffle mode of S3.3).
    Shuffle,
}

impl SortKey {
    /// Every key, in menu order.
    pub const ALL: [Self; 4] = [Self::Name, Self::Duration, Self::DateAdded, Self::Shuffle];

    /// The menu label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "Sort by Name",
            Self::Duration => "Sort by Duration",
            Self::DateAdded => "Sort by Date Added",
            Self::Shuffle => "Shuffle Once",
        }
    }
}

impl Queue {
    /// Sorts the entries by `key`; `duration` tells how long an entry's media is, when known.
    /// Equal entries keep their order.
    pub fn sort(&mut self, key: SortKey, duration: impl Fn(&Media) -> Option<Duration>) {
        self.reorder(|entries| match key {
            SortKey::Name => entries.sort_by(by_name),
            SortKey::Duration => entries.sort_by(|a, b| by_duration(&duration, a, b)),
            SortKey::DateAdded => entries.sort_by_key(|entry| entry.id.0),
            SortKey::Shuffle => Random::from_clock().shuffle(entries),
        });
    }
}

fn by_name(a: &Entry, b: &Entry) -> Ordering {
    natural_cmp(&a.media.display_name(), &b.media.display_name())
}

fn by_duration(duration: impl Fn(&Media) -> Option<Duration>, a: &Entry, b: &Entry) -> Ordering {
    match (duration(&a.media), duration(&b.media)) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn queue(names: &[&str]) -> Queue {
        let mut queue = Queue::default();
        queue.replace(names.iter().map(|name| Media::File(PathBuf::from(name))).collect());
        queue
    }

    fn names(queue: &Queue) -> Vec<String> {
        queue.entries().iter().map(|entry| entry.media.display_name()).collect()
    }

    fn length(media: &Media) -> Option<Duration> {
        let name = media.display_name();
        name.strip_prefix('t').and_then(|secs| secs.parse().ok()).map(Duration::from_secs)
    }

    #[test]
    fn sorts_names_naturally_and_keeps_the_current_entry() {
        let mut q = queue(&["Ep 10", "ep 2", "Ep 1"]);
        q.set_current(0);
        q.sort(SortKey::Name, length);
        assert_eq!(names(&q), ["Ep 1", "ep 2", "Ep 10"]);
        assert_eq!(q.current_index(), Some(2));
    }

    #[test]
    fn unknown_durations_go_last() {
        let mut q = queue(&["t30", "live", "t5", "t30"]);
        let first_30 = q.entries()[0].id;
        q.sort(SortKey::Duration, length);
        assert_eq!(names(&q), ["t5", "t30", "t30", "live"]);
        assert_eq!(q.entries()[1].id, first_30, "a stable sort");
    }

    #[test]
    fn date_added_undoes_other_sorts() {
        let mut q = queue(&["b", "a"]);
        q.append(vec![Media::File(PathBuf::from("0"))]);
        q.sort(SortKey::Name, length);
        assert_eq!(names(&q), ["0", "a", "b"]);
        q.sort(SortKey::DateAdded, length);
        assert_eq!(names(&q), ["b", "a", "0"]);
    }
}
