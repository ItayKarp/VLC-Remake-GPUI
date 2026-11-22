//! Catches a muted helper player's pictures (the seek preview, S2.3.3; library thumbnails,
//! S4.4.1): small BGRA copies that fit a box, the newest one kept until its owner takes it. The
//! owner hears about each through the `shown` callback.

use std::ffi::c_void;
use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::engine::preview::{self, PreviewImage};
use crate::engine::video::{Format, MAX_PLANES, VideoSink};

const BYTES_PER_PIXEL: u32 = 4;

/// Runs on libVLC's video thread after each picture; it must not call libVLC.
pub type Shown = Box<dyn Fn() + Send + Sync>;

/// A [`VideoSink`] that keeps the newest picture, scaled to fit a box.
pub struct FrameSink {
    /// The largest picture asked for: width and height.
    bounds: (u32, u32),
    shown: Shown,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    width: u32,
    height: u32,
    /// What libVLC writes the next picture into.
    buffer: Vec<u8>,
    /// The newest picture shown, until the owner takes it.
    newest: Option<PreviewImage>,
}

impl FrameSink {
    /// A sink whose pictures fit `bounds` (width, height), calling `shown` after each.
    pub fn new(bounds: (u32, u32), shown: Shown) -> Self {
        Self { bounds, shown, state: Mutex::default() }
    }

    /// Takes the newest picture shown, if one arrived since the last call.
    pub fn take(&self) -> Option<PreviewImage> {
        self.state().newest.take()
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl VideoSink for FrameSink {
    fn setup(&self, format: &mut Format) -> bool {
        let (width, height) = format.chosen_size();
        let (width, height) = preview::fit_within(width, height, self.bounds);
        format.chroma = *b"BGRA";
        format.width = width;
        format.height = height;
        format.pitches[0] = width * BYTES_PER_PIXEL;
        format.lines[0] = height;
        let mut state = self.state();
        state.width = width;
        state.height = height;
        state.buffer = vec![0; (width * height * BYTES_PER_PIXEL) as usize];
        true
    }

    fn lock(&self, planes: &mut [*mut c_void; MAX_PLANES]) {
        // The buffer is only reallocated in `setup`, which libVLC never runs between `lock` and
        // `unlock`, so the pointer stays valid while libVLC writes.
        planes[0] = self.state().buffer.as_mut_ptr().cast();
    }

    fn unlock(&self) {}

    fn display(&self) {
        let mut state = self.state();
        let image =
            PreviewImage { width: state.width, height: state.height, bgra: state.buffer.clone() };
        state.newest = Some(image);
        drop(state);
        (self.shown)();
    }

    fn cleanup(&self) {
        self.state().newest = None;
    }
}
