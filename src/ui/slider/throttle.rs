//! Rate limiting for held keys and drags, so a slider doesn't flood the engine with changes; the
//! effects use it too (S7.1.6), to apply a dragged value about 30 times a second.

use std::time::{Duration, Instant};

/// Lets at most one value through per `interval`, keeping the latest of the rest
/// for [`Self::flush`].
#[derive(Debug)]
pub struct Throttle<T = f64> {
    interval: Duration,
    last: Option<Instant>,
    pending: Option<T>,
}

impl<T> Throttle<T> {
    /// Creates a throttle that lets the first value through at once.
    pub fn new(interval: Duration) -> Self {
        Self { interval, last: None, pending: None }
    }

    /// Returns the value to send now, if any.
    pub fn offer(&mut self, value: T, now: Instant) -> Option<T> {
        if self.last.is_some_and(|last| now.duration_since(last) < self.interval) {
            self.pending = Some(value);
            return None;
        }
        self.last = Some(now);
        self.pending = None;
        Some(value)
    }

    /// How long after `now` a held-back value may go: the rest of the interval since the last
    /// send.
    pub fn wait(&self, now: Instant) -> Duration {
        self.last.map_or(Duration::ZERO, |last| {
            self.interval.saturating_sub(now.saturating_duration_since(last))
        })
    }

    /// The value held back since the last send, if any.
    pub fn flush(&mut self) -> Option<T> {
        self.pending.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn throttle_holds_back_bursts_and_flushes_the_latest() {
        let mut t = Throttle::new(Duration::from_millis(50));
        let t0 = Instant::now();
        assert_eq!(t.offer(1., t0), Some(1.));
        assert_eq!(t.offer(2., t0 + Duration::from_millis(10)), None);
        assert_eq!(t.offer(3., t0 + Duration::from_millis(20)), None);
        assert_eq!(t.flush(), Some(3.));
        assert_eq!(t.flush(), None);
        assert_eq!(t.offer(4., t0 + Duration::from_millis(60)), Some(4.));
    }

    #[test]
    fn the_wait_is_what_is_left_of_the_interval() {
        let mut t = Throttle::new(Duration::from_millis(33));
        let t0 = Instant::now();
        assert_eq!(t.wait(t0), Duration::ZERO);
        assert_eq!(t.offer("a", t0), Some("a"));
        assert_eq!(t.wait(t0 + Duration::from_millis(13)), Duration::from_millis(20));
        assert_eq!(t.wait(t0 + Duration::from_millis(40)), Duration::ZERO);
    }
}
