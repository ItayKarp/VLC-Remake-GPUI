//! What Previous does (S2.2.3), apart from the window so the rule can be tested.

use std::time::Duration;

use crate::queue::Queue;

/// How far into an item Previous restarts it rather than going to the item before.
pub const RESTART_AFTER: Duration = Duration::from_secs(3);

/// What Previous does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Back {
    /// Plays the current item again from its start.
    Restart,
    /// Plays item `index`.
    Item(usize),
}

/// What Previous does `position` into the current item: restart it past [`RESTART_AFTER`],
/// else go to the item before. `None` when it would do nothing.
pub fn back(queue: &Queue, position: Duration) -> Option<Back> {
    if !queue.has_current() {
        return None;
    }
    if position > RESTART_AFTER {
        return Some(Back::Restart);
    }
    queue.previous().map(Back::Item)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::app::media::Media;
    use crate::queue::Repeat;

    fn queue(current: usize, len: usize, repeat: Repeat) -> Queue {
        let mut queue = Queue::default();
        queue.set_repeat(repeat);
        queue.replace((0..len).map(|i| Media::File(PathBuf::from(i.to_string()))).collect());
        queue.set_current(current);
        queue
    }

    #[test]
    fn previous_restarts_after_three_seconds() {
        let at = |secs| back(&queue(1, 3, Repeat::Off), Duration::from_secs_f32(secs));
        assert_eq!(at(3.5), Some(Back::Restart));
        assert_eq!(at(3.0), Some(Back::Item(0)));
        assert_eq!(at(0.2), Some(Back::Item(0)));
    }

    #[test]
    fn previous_at_the_start_restarts_or_does_nothing() {
        let first = queue(0, 3, Repeat::Off);
        assert_eq!(back(&first, Duration::from_secs(10)), Some(Back::Restart));
        assert_eq!(back(&first, Duration::from_secs(1)), None);
        let wrapping = queue(0, 3, Repeat::All);
        assert_eq!(back(&wrapping, Duration::from_secs(1)), Some(Back::Item(2)));
    }

    #[test]
    fn nothing_playing_goes_nowhere() {
        assert_eq!(back(&Queue::default(), Duration::from_secs(10)), None);
    }
}
