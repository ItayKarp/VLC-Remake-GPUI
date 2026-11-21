//! When the session is written (S3.4.1): soon after any change to the queue, its current item,
//! Repeat, Shuffle or the playback state, and every so often while playing, since the position
//! moves all the time. Starting to play an item is written at once: if it crashes the decoder,
//! the next launch must know it was playing, and which item it was.

use std::time::Duration;

use crate::queue::Repeat;

/// How long a change waits before it's written, so a burst of edits is one write.
pub const DEBOUNCE: Duration = Duration::from_secs(1);
/// How often the position is written while playing. Pausing, stopping and quitting write it
/// exactly; this only bounds what a crash loses.
pub const POSITION_INTERVAL: Duration = Duration::from_secs(15);

/// What a save would record, cheaply comparable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Key {
    /// The queue's revision.
    pub revision: u64,
    /// The current item.
    pub current: Option<usize>,
    /// The repeat mode.
    pub repeat: Repeat,
    /// Whether Shuffle is on.
    pub shuffle: bool,
    /// Whether the current item is playing.
    pub playing: bool,
    /// The position, in whole seconds.
    pub second: u64,
}

/// When to write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum When {
    /// Nothing worth writing changed.
    Not,
    /// After [`DEBOUNCE`].
    Soon,
    /// Right away.
    Now,
}

/// When to write `now`, given what was last written and how long ago.
pub fn when(written: Key, now: Key, since_written: Duration) -> When {
    if now.playing && (!written.playing || now.current != written.current) {
        return When::Now;
    }
    let moved = Key { second: written.second, ..now } != written;
    let seeked = now.second != written.second;
    if moved || (seeked && (!now.playing || since_written >= POSITION_INTERVAL)) {
        When::Soon
    } else {
        When::Not
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAYING: Key = Key {
        revision: 1,
        current: Some(0),
        repeat: Repeat::Off,
        shuffle: false,
        playing: true,
        second: 10,
    };
    const A_MOMENT: Duration = Duration::from_secs(2);

    #[test]
    fn starting_to_play_is_written_at_once() {
        let paused = Key { playing: false, ..PLAYING };
        assert_eq!(when(paused, PLAYING, A_MOMENT), When::Now);
    }

    #[test]
    fn playing_another_item_is_written_at_once() {
        let next = Key { current: Some(1), second: 0, ..PLAYING };
        assert_eq!(when(PLAYING, next, A_MOMENT), When::Now);
    }

    #[test]
    fn edits_and_pauses_are_written_soon() {
        let edited = Key { revision: 2, ..PLAYING };
        assert_eq!(when(PLAYING, edited, A_MOMENT), When::Soon);
        let paused = Key { playing: false, second: 11, ..PLAYING };
        assert_eq!(when(PLAYING, paused, A_MOMENT), When::Soon);
        let shuffled = Key { shuffle: true, ..PLAYING };
        assert_eq!(when(PLAYING, shuffled, A_MOMENT), When::Soon);
    }

    #[test]
    fn the_playing_position_is_written_every_interval() {
        let later = Key { second: 12, ..PLAYING };
        assert_eq!(when(PLAYING, later, A_MOMENT), When::Not);
        assert_eq!(when(PLAYING, later, POSITION_INTERVAL), When::Soon);
        assert_eq!(when(PLAYING, PLAYING, POSITION_INTERVAL), When::Not);
    }

    #[test]
    fn a_seek_while_paused_is_written_soon() {
        let paused = Key { playing: false, ..PLAYING };
        let seeked = Key { second: 300, ..paused };
        assert_eq!(when(paused, seeked, A_MOMENT), When::Soon);
    }
}
