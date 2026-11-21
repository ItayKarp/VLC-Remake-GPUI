//! A one-frame mailbox from libVLC's video thread to the GPUI window.
//!
//! The video thread publishes; the window takes the newest frame when it next draws. A frame
//! replaced before the window took it was decoded but never shown, and is counted as such.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};

/// A frame and the moment libVLC said it was due.
pub struct Stamped<T> {
    /// The frame.
    pub frame: T,
    /// When the `display` callback published it.
    pub due: Instant,
}

/// The newest frame, plus counters for what happened to the others.
pub struct FrameSlot<T> {
    latest: Mutex<Option<Stamped<T>>>,
    wake: UnboundedSender<()>,
    wake_pending: AtomicBool,
    published: AtomicU64,
    replaced: AtomicU64,
}

impl<T> FrameSlot<T> {
    /// A new slot and the receiver that ticks once per batch of published frames.
    pub fn new() -> (Self, UnboundedReceiver<()>) {
        let (wake, woken) = unbounded();
        let slot = Self {
            latest: Mutex::new(None),
            wake,
            wake_pending: AtomicBool::new(false),
            published: AtomicU64::new(0),
            replaced: AtomicU64::new(0),
        };
        (slot, woken)
    }

    /// Makes `frame` the newest, waking the window unless a wake-up is already queued.
    pub fn publish(&self, frame: T) {
        let stamped = Stamped { frame, due: Instant::now() };
        let previous = self.latest.lock().expect("slot lock poisoned").replace(stamped);
        self.published.fetch_add(1, Ordering::Relaxed);
        if previous.is_some() {
            self.replaced.fetch_add(1, Ordering::Relaxed);
        }
        if !self.wake_pending.swap(true, Ordering::AcqRel) {
            // The window may already be gone during shutdown; then nobody needs waking.
            self.wake.unbounded_send(()).ok();
        }
    }

    /// Takes the newest frame, if one arrived since the last call.
    pub fn take(&self) -> Option<Stamped<T>> {
        self.wake_pending.store(false, Ordering::Release);
        self.latest.lock().expect("slot lock poisoned").take()
    }

    /// Frames published so far, and how many of them were replaced unseen.
    pub fn counts(&self) -> (u64, u64) {
        (self.published.load(Ordering::Relaxed), self.replaced.load(Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::FrameSlot;

    #[test]
    fn take_returns_the_newest_frame_once() {
        let (slot, _woken) = FrameSlot::new();
        slot.publish(1);
        slot.publish(2);
        assert_eq!(slot.take().map(|s| s.frame), Some(2));
        assert!(slot.take().is_none());
        assert_eq!(slot.counts(), (2, 1));
    }

    #[test]
    fn wakes_once_per_batch() {
        let (slot, mut woken) = FrameSlot::new();
        slot.publish(1);
        slot.publish(2);
        assert!(woken.try_recv().is_ok(), "one wake-up queued");
        assert!(woken.try_recv().is_err(), "the second frame must not queue another");
        slot.take();
        slot.publish(3);
        assert!(woken.try_recv().is_ok(), "a new batch wakes again");
    }
}
