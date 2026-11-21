//! Where Previous and Next go (S2.2.3), apart from the window so the rules can be tested.

use std::time::Duration;

/// How far into an item Previous restarts it rather than going to the item before.
pub const RESTART_AFTER: Duration = Duration::from_secs(3);

/// What Previous and Next do at the queue's ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ends {
    /// They stop there: Previous on the first item and Next on the last do nothing.
    Stop,
    /// They wrap around to the other end (Repeat All, once the queue has it: S3.1).
    Wrap,
}

/// Where the play list stands, as Previous and Next see it.
#[derive(Clone, Copy, Debug)]
pub struct Queue {
    /// The index of the item the player has, if any.
    pub current: Option<usize>,
    /// How many items the list holds.
    pub len: usize,
    /// What happens at the ends.
    pub ends: Ends,
}

/// What Previous does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Back {
    /// Plays the current item again from its start.
    Restart,
    /// Plays item `index`.
    Item(usize),
}

impl Queue {
    /// The item Next plays, or `None` at the end of the list (unless it wraps).
    pub fn next(self) -> Option<usize> {
        let current = self.current?;
        match current + 1 {
            next if next < self.len => Some(next),
            _ if self.ends == Ends::Wrap && self.len > 0 => Some(0),
            _ => None,
        }
    }

    /// The item before the current one, or `None` at the start of the list (unless it wraps).
    pub fn previous(self) -> Option<usize> {
        let current = self.current?;
        match current.checked_sub(1) {
            Some(previous) => Some(previous),
            None if self.ends == Ends::Wrap && self.len > 0 => Some(self.len - 1),
            None => None,
        }
    }

    /// What Previous does `position` into the current item: restart it past
    /// [`RESTART_AFTER`], else go to the item before. `None` when it would do nothing.
    pub fn back(self, position: Duration) -> Option<Back> {
        self.current?;
        if position > RESTART_AFTER {
            return Some(Back::Restart);
        }
        self.previous().map(Back::Item)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn queue(current: usize, len: usize, ends: Ends) -> Queue {
        Queue { current: Some(current), len, ends }
    }

    #[test]
    fn next_stops_at_the_end_unless_it_wraps() {
        assert_eq!(queue(0, 3, Ends::Stop).next(), Some(1));
        assert_eq!(queue(2, 3, Ends::Stop).next(), None);
        assert_eq!(queue(2, 3, Ends::Wrap).next(), Some(0));
    }

    #[test]
    fn previous_restarts_after_three_seconds() {
        let at = |secs| queue(1, 3, Ends::Stop).back(Duration::from_secs_f32(secs));
        assert_eq!(at(3.5), Some(Back::Restart));
        assert_eq!(at(3.0), Some(Back::Item(0)));
        assert_eq!(at(0.2), Some(Back::Item(0)));
    }

    #[test]
    fn previous_at_the_start_restarts_or_does_nothing() {
        let first = queue(0, 3, Ends::Stop);
        assert_eq!(first.back(Duration::from_secs(10)), Some(Back::Restart));
        assert_eq!(first.back(Duration::from_secs(1)), None);
        assert_eq!(queue(0, 3, Ends::Wrap).back(Duration::from_secs(1)), Some(Back::Item(2)));
    }

    #[test]
    fn nothing_playing_goes_nowhere() {
        let empty = Queue { current: None, len: 0, ends: Ends::Wrap };
        assert_eq!(empty.next(), None);
        assert_eq!(empty.back(Duration::from_secs(10)), None);
    }

    #[test]
    fn a_single_wrapping_item_repeats_itself() {
        assert_eq!(queue(0, 1, Ends::Wrap).next(), Some(0));
        assert_eq!(queue(0, 1, Ends::Wrap).previous(), Some(0));
    }
}
