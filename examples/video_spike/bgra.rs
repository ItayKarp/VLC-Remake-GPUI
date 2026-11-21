//! Presentation path (a): libVLC converts to BGRA in CPU memory, and each frame becomes a
//! GPUI `RenderImage` that GPUI uploads into its texture atlas. Works on every OS.

use std::ffi::c_void;
use std::sync::{Arc, Mutex};

use gpui::RenderImage;
use image::{Frame, RgbaImage};

use crate::sink::{Format, MAX_PLANES, Size, VideoSink, choose_size};
use crate::slot::FrameSlot;

const BYTES_PER_PIXEL: u32 = 4;

/// Turns libVLC pictures into `RenderImage`s.
pub struct BgraSink {
    slot: Arc<FrameSlot<Arc<RenderImage>>>,
    size: Size,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    width: u32,
    height: u32,
    /// The picture libVLC is writing (between `lock` and `display`).
    writing: Option<Vec<u8>>,
}

impl BgraSink {
    /// A sink publishing into `slot`, sizing pictures by `size`.
    pub fn new(slot: Arc<FrameSlot<Arc<RenderImage>>>, size: Size) -> Self {
        Self { slot, size, state: Mutex::default() }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().expect("BGRA sink lock poisoned")
    }
}

impl VideoSink for BgraSink {
    fn setup(&self, format: &mut Format) -> bool {
        let (width, height) = choose_size(format, self.size.display, self.size.max_texture);
        format.chroma = *b"BGRA";
        format.width = width;
        format.height = height;
        format.pitches[0] = width * BYTES_PER_PIXEL;
        format.lines[0] = height;
        let mut state = self.state();
        state.width = width;
        state.height = height;
        true
    }

    fn lock(&self, planes: &mut [*mut c_void; MAX_PLANES]) {
        let mut state = self.state();
        let len = (state.width * state.height * BYTES_PER_PIXEL) as usize;
        // A fresh buffer per frame: the `RenderImage` takes ownership of it, so nothing can
        // be recycled. That allocation is part of what this path costs.
        let mut buffer = vec![0_u8; len];
        planes[0] = buffer.as_mut_ptr().cast();
        state.writing = Some(buffer);
    }

    fn unlock(&self) {}

    fn display(&self) {
        let mut state = self.state();
        let Some(buffer) = state.writing.take() else { return };
        // GPUI's `RenderImage` holds BGRA bytes in an `RgbaImage` container.
        let Some(image) = RgbaImage::from_raw(state.width, state.height, buffer) else { return };
        drop(state);
        self.slot.publish(Arc::new(RenderImage::new([Frame::new(image)])));
    }
}
