//! Command coalescing (S1.2.5): rapid seeks keep only the latest target, and frame steps queue
//! up to [`MAX_QUEUED_STEPS`].

use std::collections::VecDeque;

use super::values::{BackendSeek, StepDirection};

/// How many frame steps may wait behind the one in flight (holding the step key).
pub const MAX_QUEUED_STEPS: usize = 8;

/// Sends one seek at a time to the backend. While one is in flight, or before the media says
/// whether it is seekable, later seeks replace each other and only the latest is sent.
#[derive(Debug, Default)]
pub struct SeekQueue {
    /// Whether the media is seekable; `None` until the backend says.
    seekable: Option<bool>,
    in_flight: bool,
    pending: Option<BackendSeek>,
}

impl SeekQueue {
    /// Asks for `seek`. Returns it when it should go to the backend now.
    pub fn request(&mut self, seek: BackendSeek) -> Option<BackendSeek> {
        match self.seekable {
            Some(false) => None,
            Some(true) if !self.in_flight => {
                self.in_flight = true;
                Some(seek)
            }
            _ => {
                self.pending = Some(seek);
                None
            }
        }
    }

    /// The backend finished the seek in flight. Returns the next one to send, if any.
    pub fn completed(&mut self) -> Option<BackendSeek> {
        self.in_flight = false;
        self.send_pending()
    }

    /// The media became seekable or not. A seek waiting for it is sent, or dropped when the
    /// media can't seek (live streams).
    pub fn set_seekable(&mut self, seekable: bool) -> Option<BackendSeek> {
        self.seekable = Some(seekable);
        if seekable {
            return self.send_pending();
        }
        self.pending = None;
        None
    }

    /// Whether a seek is in flight or waiting, so positions the backend reports are stale.
    pub fn is_busy(&self) -> bool {
        self.in_flight || self.pending.is_some()
    }

    /// Whether the media is known to be seekable.
    pub fn is_seekable(&self) -> Option<bool> {
        self.seekable
    }

    /// Forgets everything, for a new open or when playback ends.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Forgets queued seeks but keeps what is known about the media.
    pub fn cancel(&mut self) {
        self.in_flight = false;
        self.pending = None;
    }

    fn send_pending(&mut self) -> Option<BackendSeek> {
        if self.in_flight || self.seekable != Some(true) {
            return None;
        }
        let seek = self.pending.take()?;
        self.in_flight = true;
        Some(seek)
    }
}

/// Sends one frame step at a time, queuing up to [`MAX_QUEUED_STEPS`] more.
#[derive(Debug, Default)]
pub struct FrameSteps {
    in_flight: bool,
    queued: VecDeque<StepDirection>,
}

impl FrameSteps {
    /// Asks for a step. Returns it when it should go to the backend now; a step past a full
    /// queue is dropped.
    pub fn request(&mut self, direction: StepDirection) -> Option<StepDirection> {
        if !self.in_flight {
            self.in_flight = true;
            return Some(direction);
        }
        if self.queued.len() < MAX_QUEUED_STEPS {
            self.queued.push_back(direction);
        }
        None
    }

    /// The backend finished a step. Returns the next one to send, if any.
    pub fn completed(&mut self) -> Option<StepDirection> {
        let next = self.queued.pop_front();
        self.in_flight = next.is_some();
        next
    }

    /// Drops every queued step.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn at(secs: u64) -> BackendSeek {
        BackendSeek::Time(Duration::from_secs(secs))
    }

    #[test]
    fn rapid_seeks_keep_only_the_latest_target() {
        let mut queue = SeekQueue::default();
        queue.set_seekable(true);
        assert_eq!(queue.request(at(1)), Some(at(1)));
        assert_eq!(queue.request(at(2)), None);
        assert_eq!(queue.request(at(3)), None);
        assert_eq!(queue.completed(), Some(at(3)));
        assert_eq!(queue.completed(), None);
        assert!(!queue.is_busy());
    }

    #[test]
    fn a_seek_before_seekable_waits_for_it() {
        let mut queue = SeekQueue::default();
        assert_eq!(queue.request(at(5)), None);
        assert!(queue.is_busy());
        assert_eq!(queue.set_seekable(true), Some(at(5)));
    }

    #[test]
    fn a_seek_is_dropped_when_the_media_cannot_seek() {
        let mut queue = SeekQueue::default();
        queue.request(at(5));
        assert_eq!(queue.set_seekable(false), None);
        assert!(!queue.is_busy());
        assert_eq!(queue.request(at(6)), None);
        assert!(!queue.is_busy());
    }

    #[test]
    fn frame_steps_queue_up_to_the_limit() {
        let mut steps = FrameSteps::default();
        assert_eq!(steps.request(StepDirection::Forward), Some(StepDirection::Forward));
        for _ in 0..MAX_QUEUED_STEPS + 3 {
            assert_eq!(steps.request(StepDirection::Backward), None);
        }
        let sent = std::iter::from_fn(|| steps.completed()).count();
        assert_eq!(sent, MAX_QUEUED_STEPS);
        assert_eq!(steps.request(StepDirection::Forward), Some(StepDirection::Forward));
    }
}
