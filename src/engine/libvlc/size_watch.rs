//! Watches for a video size that only decoding reveals (S1.4.1, ADR 0001 limit 2).
//!
//! MPEG-PS and MPEG-TS describe their video track as 0×0 until the decoder has read the first
//! pictures, and libVLC 3 sends no event when the size and pixel aspect then become known
//! (`dvd-anamorphic.mpg` learns its 32:27 pixels about 150 ms into playback). So while a video
//! track has no size, the worker reads it again every [`SIZE_POLL`], for at most
//! [`SIZE_WATCH`].

use std::cell::Cell;
use std::time::{Duration, Instant};

/// How often to read the size again while it is unknown.
pub const SIZE_POLL: Duration = Duration::from_millis(100);
/// How long to keep trying before accepting that the size stays unknown.
const SIZE_WATCH: Duration = Duration::from_secs(3);

/// Whether the worker is waiting for the video size, and until when.
#[derive(Debug, Default)]
pub struct SizeWatch {
    until: Cell<Option<Instant>>,
}

impl SizeWatch {
    /// Starts watching when `unknown` (the media has video, of unknown size); stops once it
    /// isn't (the size is known, or there is no video).
    pub fn observe(&self, unknown: bool, now: Instant) {
        if !unknown {
            self.until.set(None);
        } else if self.until.get().is_none() {
            self.until.set(Some(now + SIZE_WATCH));
        }
    }

    /// Stops watching (a new media is opening).
    pub fn stop(&self) {
        self.until.set(None);
    }

    /// Whether the size should be read again at `now`.
    pub fn is_watching(&self, now: Instant) -> bool {
        self.until.get().is_some_and(|until| now < until)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watches_video_of_unknown_size_until_it_is_known() {
        let watch = SizeWatch::default();
        let start = Instant::now();
        watch.observe(true, start);
        assert!(watch.is_watching(start + SIZE_POLL));
        watch.observe(false, start + SIZE_POLL);
        assert!(!watch.is_watching(start + SIZE_POLL));
    }

    #[test]
    fn gives_up_after_the_watch_and_ignores_audio() {
        let watch = SizeWatch::default();
        let start = Instant::now();
        watch.observe(true, start);
        // A later unknown report doesn't extend the deadline.
        watch.observe(true, start + SIZE_WATCH / 2);
        assert!(!watch.is_watching(start + SIZE_WATCH));
        watch.stop();
        watch.observe(false, start);
        assert!(!watch.is_watching(start));
    }
}
