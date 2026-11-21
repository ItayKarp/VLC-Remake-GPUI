//! A one-frame mailbox from the backend's video thread to the window (ADR 0001, limit 8).
//!
//! The video thread publishes; the window takes the newest frame when it next draws, so a slow
//! frame never queues more behind it. While the window is hidden the slot refuses frames
//! (S1.4.4): they are decoded, for audio and A/V sync, but never uploaded or drawn.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};

/// What the window should show next.
#[derive(Debug, PartialEq, Eq)]
pub enum Delivery<T> {
    /// A new frame.
    Frame(T),
    /// The video output closed: show nothing.
    Cleared,
}

/// The newest delivery, and whether frames are wanted.
pub struct FrameSlot<T> {
    latest: Mutex<Option<Delivery<T>>>,
    wake: UnboundedSender<()>,
    wake_pending: AtomicBool,
    accepting: AtomicBool,
}

impl<T> FrameSlot<T> {
    /// A new slot, accepting frames, and the receiver that ticks once per batch of deliveries.
    pub fn new() -> (Self, UnboundedReceiver<()>) {
        let (wake, woken) = unbounded();
        let slot = Self {
            latest: Mutex::new(None),
            wake,
            wake_pending: AtomicBool::new(false),
            accepting: AtomicBool::new(true),
        };
        (slot, woken)
    }

    /// Makes `frame` the newest, unless frames are refused (see [`Self::set_accepting`]).
    pub fn publish(&self, frame: T) {
        if self.is_accepting() {
            self.deliver(Delivery::Frame(frame));
        }
    }

    /// Replaces whatever is pending with "show nothing".
    pub fn clear(&self) {
        self.deliver(Delivery::Cleared);
    }

    /// Takes the newest delivery, if one arrived since the last call.
    pub fn take(&self) -> Option<Delivery<T>> {
        self.wake_pending.store(false, Ordering::Release);
        self.latest().take()
    }

    /// Whether published frames are kept; sinks may skip work for frames that would be dropped.
    pub fn is_accepting(&self) -> bool {
        self.accepting.load(Ordering::Relaxed)
    }

    /// Starts or stops keeping frames. Stopping drops a frame that is waiting.
    pub fn set_accepting(&self, accepting: bool) {
        self.accepting.store(accepting, Ordering::Relaxed);
        if !accepting {
            let mut latest = self.latest();
            if matches!(*latest, Some(Delivery::Frame(_))) {
                *latest = None;
            }
        }
    }

    fn deliver(&self, delivery: Delivery<T>) {
        *self.latest() = Some(delivery);
        if !self.wake_pending.swap(true, Ordering::AcqRel) {
            // The window may already be gone during shutdown; then nobody needs waking.
            self.wake.unbounded_send(()).ok();
        }
    }

    fn latest(&self) -> MutexGuard<'_, Option<Delivery<T>>> {
        self.latest.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn take_returns_the_newest_frame_once() {
        let (slot, _woken) = FrameSlot::new();
        slot.publish(1);
        slot.publish(2);
        assert_eq!(slot.take(), Some(Delivery::Frame(2)));
        assert_eq!(slot.take(), None);
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

    #[test]
    fn clearing_replaces_a_pending_frame() {
        let (slot, _woken) = FrameSlot::new();
        slot.publish(1);
        slot.clear();
        assert_eq!(slot.take(), Some(Delivery::Cleared));
    }

    #[test]
    fn a_hidden_window_gets_no_frames_but_still_hears_of_a_clear() {
        let (slot, _woken) = FrameSlot::new();
        slot.publish(1);
        slot.set_accepting(false);
        assert_eq!(slot.take(), None, "the waiting frame is dropped");
        slot.publish(2);
        assert_eq!(slot.take(), None);
        slot.clear();
        assert_eq!(slot.take(), Some(Delivery::Cleared));
        slot.set_accepting(true);
        slot.publish(3);
        assert_eq!(slot.take(), Some(Delivery::Frame(3)));
    }
}
