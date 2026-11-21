//! Presentation path (a), the fallback on Windows and Linux (ADR 0001): the backend converts
//! each picture to BGRA in CPU memory, and it becomes a GPUI `RenderImage`.

use std::ffi::c_void;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use gpui::RenderImage;
use image::{Frame as ImageFrame, RgbaImage};

use super::frame::Frame;
use super::frame_slot::FrameSlot;
use crate::engine::video::{Format, MAX_PLANES, VideoSink};

const BYTES_PER_PIXEL: u32 = 4;

/// Turns pictures into `RenderImage`s.
pub struct BgraSink {
    slot: Arc<FrameSlot<Frame>>,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    width: u32,
    height: u32,
    /// The picture being written, between `lock` and `display`.
    writing: Option<Vec<u8>>,
}

impl BgraSink {
    /// A sink publishing into `slot`.
    pub fn new(slot: Arc<FrameSlot<Frame>>) -> Self {
        Self { slot, state: Mutex::default() }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl VideoSink for BgraSink {
    fn setup(&self, format: &mut Format) -> bool {
        let (width, height) = format.chosen_size();
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
        // A buffer per frame: the `RenderImage` takes ownership of it, so nothing is recycled.
        // A frame the hidden window won't take keeps its buffer for the next one.
        let mut buffer = state.writing.take().filter(|old| old.len() == len);
        let buffer = buffer.get_or_insert_with(|| vec![0_u8; len]);
        planes[0] = buffer.as_mut_ptr().cast();
        state.writing = Some(std::mem::take(buffer));
    }

    fn unlock(&self) {}

    fn display(&self) {
        if !self.slot.is_accepting() {
            return;
        }
        let mut state = self.state();
        let Some(buffer) = state.writing.take() else { return };
        // GPUI's `RenderImage` holds BGRA bytes in an `RgbaImage` container.
        let Some(image) = RgbaImage::from_raw(state.width, state.height, buffer) else { return };
        drop(state);
        let image = RenderImage::new([ImageFrame::new(image)]);
        self.slot.publish(Frame::Image(Arc::new(image)));
    }

    fn cleanup(&self) {
        self.state().writing = None;
        self.slot.clear();
    }
}
