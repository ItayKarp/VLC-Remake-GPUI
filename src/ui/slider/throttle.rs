//! Rate limiting for held keys, so a slider doesn't flood the engine with changes.

use std::time::{Duration, Instant};

/// Lets at most one value through per `interval`, keeping the latest of the rest
/// for [`Self::flush`].
#[derive(Debug)]
pub struct Throttle {
    interval: Duration,
    last: Option<Instant>,
    pending: Option<f64>,
}

impl Throttle {
    /// Creates a throttle that lets the first value through at once.
    pub fn new(interval: Duration) -> Self {
        Self { interval, last: None, pending: None }
    }

    /// Returns the value to send now, if any.
    pub fn offer(&mut self, value: f64, now: Instant) -> Option<f64> {
        if self.last.is_some_and(|last| now.duration_since(last) < self.interval) {
            self.pending = Some(value);
            return None;
        }
        self.last = Some(now);
        self.pending = None;
        Some(value)
    }

    /// The value held back since the last send, if any.
    pub fn flush(&mut self) -> Option<f64> {
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
}
