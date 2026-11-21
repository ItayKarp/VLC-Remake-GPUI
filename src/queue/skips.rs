//! Skipping items that fail to play (S3.1.4), without spinning through a broken queue: after
//! [`MAX_CONSECUTIVE_FAILURES`] failures in a row the queue stops, whatever Repeat says.

/// How many items in a row may fail before the queue stops skipping.
pub const MAX_CONSECUTIVE_FAILURES: u32 = 5;

/// What to do after an item failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AfterError {
    /// Play the next item.
    Skip,
    /// Stop: too many items in a row failed.
    GiveUp,
}

/// Counts the failures since something last played.
#[derive(Clone, Copy, Debug, Default)]
pub struct Skips {
    failures: u32,
}

impl Skips {
    /// Something played: the count starts over.
    pub fn played(&mut self) {
        self.failures = 0;
    }

    /// An item failed; returns whether to skip to the next one.
    pub fn failed(&mut self) -> AfterError {
        self.failures += 1;
        if self.failures >= MAX_CONSECUTIVE_FAILURES {
            self.failures = 0;
            AfterError::GiveUp
        } else {
            AfterError::Skip
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gives_up_after_five_failures_in_a_row() {
        let mut skips = Skips::default();
        for _ in 1..MAX_CONSECUTIVE_FAILURES {
            assert_eq!(skips.failed(), AfterError::Skip);
        }
        assert_eq!(skips.failed(), AfterError::GiveUp);
        // Playing something again (the user picks another item) starts a fresh count.
        assert_eq!(skips.failed(), AfterError::Skip);
    }

    #[test]
    fn playing_resets_the_count() {
        let mut skips = Skips::default();
        for _ in 0..10 {
            assert_eq!(skips.failed(), AfterError::Skip);
            skips.played();
        }
    }
}
