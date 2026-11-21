//! Catches the preview player's pictures: small BGRA copies, the newest one kept for the
//! worker, which hears about each through its inbox.

use std::ffi::c_void;
use std::sync::{Mutex, MutexGuard, PoisonError, mpsc};

use super::worker::Message;
use crate::engine::preview::{self, PreviewImage};
use crate::engine::video::{Format, MAX_PLANES, VideoSink};

const BYTES_PER_PIXEL: u32 = 4;

/// The preview player's [`VideoSink`].
pub struct PreviewSink {
    worker: mpsc::Sender<Message>,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    width: u32,
    height: u32,
    /// What libVLC writes the next picture into.
    buffer: Vec<u8>,
    /// The newest picture shown, until the worker takes it.
    shown: Option<PreviewImage>,
}

impl PreviewSink {
    /// A sink that tells `worker` about each picture.
    pub fn new(worker: mpsc::Sender<Message>) -> Self {
        Self { worker, state: Mutex::default() }
    }

    /// Takes the newest picture shown, if one arrived since the last call.
    pub fn take(&self) -> Option<PreviewImage> {
        self.state().shown.take()
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl VideoSink for PreviewSink {
    fn setup(&self, format: &mut Format) -> bool {
        let (width, height) = format.chosen_size();
        let (width, height) = preview::fit(width, height);
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
        state.shown = Some(image);
        drop(state);
        // An error means the worker exited during shutdown.
        let _ = self.worker.send(Message::Picture);
    }

    fn cleanup(&self) {
        self.state().shown = None;
    }
}
