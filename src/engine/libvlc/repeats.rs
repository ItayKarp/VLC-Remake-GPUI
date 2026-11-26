//! Drops the pictures libVLC shows again unchanged (S9.3.2).
//!
//! libVLC 3's video output draws the last picture again every 80 ms (`VOUT_REDISPLAY_DELAY`)
//! while no new one is due: all the time while paused, and between the pictures of a still or
//! very slow video. Through `vmem` each of those is a full copy, a new texture and a redraw of
//! the whole window, about 12 a second for nothing. So a picture that comes at least
//! [`HASH_AFTER`] after the one before is hashed, and one identical to that one isn't shown.
//! Pictures during normal playback come sooner and are never hashed. A redraw that changed the
//! picture (a seek or frame step while paused, a subtitle libVLC draws in) differs and shows.

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use crate::engine::video::{Format, MAX_PLANES};

/// The gap after which a picture may be a redraw: under libVLC's 80 ms, over any playing video's
/// frame interval down to about 14 frames a second.
pub const HASH_AFTER: Duration = Duration::from_millis(70);

/// Hashes the pictures that may be redraws, and says which repeat the one before.
#[derive(Debug, Default)]
pub struct Repeats {
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    /// Bytes in each plane of the agreed format.
    sizes: [usize; MAX_PLANES],
    /// Where the picture being written is, as addresses (the planes the sink lent libVLC).
    planes: [usize; MAX_PLANES],
    /// When the last picture was written.
    written: Option<Instant>,
    /// The last picture's hash, when it was hashed.
    hash: Option<u64>,
    /// Whether the picture just written repeats the one before.
    repeat: bool,
}

impl Repeats {
    /// The video output agreed on `format`: pictures from now on are laid out as it says.
    pub fn set_format(&self, format: &Format) {
        let mut state = self.state();
        *state = State::default();
        for (size, (&pitch, &lines)) in
            state.sizes.iter_mut().zip(format.pitches.iter().zip(&format.lines))
        {
            *size = pitch as usize * lines as usize;
        }
    }

    /// libVLC is about to write a picture into `planes`.
    pub fn locked(&self, planes: &[*mut std::ffi::c_void; MAX_PLANES]) {
        self.state().planes = planes.map(|plane| plane as usize);
    }

    /// libVLC wrote the picture at `now`, before the sink lets go of its planes.
    pub fn written(&self, now: Instant) {
        let mut state = self.state();
        let gap = state.written.map(|then| now.saturating_duration_since(then));
        state.written = Some(now);
        let hash = gap.is_some_and(|gap| gap >= HASH_AFTER).then(|| state.hash_planes());
        state.repeat = hash.is_some() && hash == state.hash;
        state.hash = hash;
    }

    /// Whether the picture just written repeats the one before, so needn't be shown.
    pub fn is_repeat(&self) -> bool {
        self.state().repeat
    }

    /// The output closed: the next picture starts afresh.
    pub fn reset(&self) {
        let mut state = self.state();
        *state = State { sizes: state.sizes, ..State::default() };
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl State {
    /// The hash of every plane of the picture being written.
    fn hash_planes(&self) -> u64 {
        let mut hasher = xxhash_rust::xxh3::Xxh3::new();
        for (&address, &size) in self.planes.iter().zip(&self.sizes) {
            if address == 0 || size == 0 {
                continue;
            }
            // SAFETY: the sink lent this plane for the picture being written and keeps it until
            // its `unlock`, which runs after this; the agreed format says how large it is.
            let bytes = unsafe { std::slice::from_raw_parts(address as *const u8, size) };
            hasher.update(bytes);
        }
        hasher.digest()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-plane picture of `bytes`, written at `at`, and whether it repeated.
    fn write(repeats: &Repeats, bytes: &mut [u8], at: Instant) -> bool {
        let mut planes = [std::ptr::null_mut(); MAX_PLANES];
        planes[0] = bytes.as_mut_ptr().cast();
        repeats.locked(&planes);
        repeats.written(at);
        repeats.is_repeat()
    }

    fn repeats_for(len: usize) -> Repeats {
        let repeats = Repeats::default();
        let mut format = Format::default();
        (format.pitches[0], format.lines[0]) = (len as u32, 1);
        repeats.set_format(&format);
        repeats
    }

    #[test]
    fn a_redraw_of_the_same_picture_is_a_repeat() {
        let repeats = repeats_for(4);
        let start = Instant::now();
        let redraw = |n| start + HASH_AFTER * n;
        assert!(!write(&repeats, &mut [1, 2, 3, 4], start));
        assert!(!write(&repeats, &mut [1, 2, 3, 4], redraw(1)), "the first redraw is hashed");
        assert!(write(&repeats, &mut [1, 2, 3, 4], redraw(2)));
        assert!(!write(&repeats, &mut [9, 2, 3, 4], redraw(3)), "a changed picture shows");
        assert!(write(&repeats, &mut [9, 2, 3, 4], redraw(4)));
    }

    #[test]
    fn pictures_during_playback_are_never_hashed_or_dropped() {
        let repeats = repeats_for(2);
        let start = Instant::now();
        let frame = Duration::from_millis(33);
        for n in 0..5 {
            assert!(!write(&repeats, &mut [7, 7], start + frame * n));
        }
    }

    #[test]
    fn a_new_output_starts_afresh() {
        let repeats = repeats_for(2);
        let start = Instant::now();
        write(&repeats, &mut [1, 1], start);
        write(&repeats, &mut [1, 1], start + HASH_AFTER);
        repeats.reset();
        assert!(!write(&repeats, &mut [1, 1], start + HASH_AFTER * 2));
    }
}
