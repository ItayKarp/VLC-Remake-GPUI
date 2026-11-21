//! The queue's pseudo-random numbers, for Shuffle Once (S3.2.5) and the Shuffle mode (S3.3.2).
//! A small SplitMix64 generator is plenty for picking play orders and needs no dependency.

use std::time::{SystemTime, UNIX_EPOCH};

/// A pseudo-random sequence.
#[derive(Clone, Debug)]
pub(super) struct Random {
    state: u64,
}

impl Random {
    /// A sequence that starts at `seed`, the same every time (for tests).
    pub fn seeded(seed: u64) -> Self {
        Self { state: seed }
    }

    /// A sequence that differs from one run to the next.
    pub fn from_clock() -> Self {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        Self::seeded((now.as_nanos() as u64) | 1)
    }

    /// A number in `0..=max`.
    pub fn up_to(&mut self, max: usize) -> usize {
        self.state = splitmix(self.state);
        (self.state % (max as u64 + 1)) as usize
    }

    /// Shuffles `items` (Fisher–Yates).
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for last in (1..items.len()).rev() {
            let pick = self.up_to(last);
            items.swap(last, pick);
        }
    }
}

impl Default for Random {
    fn default() -> Self {
        Self::from_clock()
    }
}

/// One step of SplitMix64: a small, well-mixed pseudo-random sequence.
fn splitmix(state: u64) -> u64 {
    let mut z = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shuffling_keeps_every_item() {
        let mut items: Vec<u32> = (0..100).collect();
        Random::seeded(42).shuffle(&mut items);
        assert_ne!(items, (0..100).collect::<Vec<_>>());
        items.sort_unstable();
        assert_eq!(items, (0..100).collect::<Vec<_>>());
        let mut one = [7];
        Random::seeded(1).shuffle(&mut one);
        assert_eq!(one, [7]);
    }

    #[test]
    fn picks_stay_in_range() {
        let mut random = Random::seeded(7);
        assert!((0..1_000).all(|_| random.up_to(3) <= 3));
        assert_eq!(random.up_to(0), 0);
    }
}
