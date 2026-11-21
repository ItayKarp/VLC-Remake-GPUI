//! The video output surface (S1.4): decoded pictures from the engine drawn inside the window.
//!
//! - [`Output`] pairs the engine-facing [`VideoSink`] of this OS's presentation path (ADR 0001:
//!   IOSurface pixel buffers on macOS, BGRA images elsewhere) with the mailbox it fills.
//! - [`frame_slot`] is that one-frame mailbox; it refuses frames while the window is hidden.
//! - [`layout`] letterboxes the picture on device pixels.
//! - [`VideoView`] draws the newest picture, black bars around it whatever the theme.

mod bgra;
mod frame;
pub mod frame_slot;
pub mod layout;
#[cfg(target_os = "macos")]
mod pixel_buffer;
mod view;

use std::sync::Arc;

use futures::channel::mpsc::UnboundedReceiver;

use self::frame::Frame;
use self::frame_slot::FrameSlot;
use crate::engine::video::VideoSink;

pub use view::VideoView;

/// A presentation path: the sink the engine writes pictures into, and the mailbox the
/// [`VideoView`] takes them from.
pub struct Output {
    slot: Arc<FrameSlot<Frame>>,
    woken: UnboundedReceiver<()>,
    sink: Arc<dyn VideoSink>,
}

impl Output {
    /// This OS's presentation path.
    pub fn new() -> Self {
        let (slot, woken) = FrameSlot::new();
        let slot = Arc::new(slot);
        Self { sink: native_sink(&slot), slot, woken }
    }

    /// The sink to hand the engine's backend.
    pub fn sink(&self) -> Arc<dyn VideoSink> {
        self.sink.clone()
    }
}

/// Set to `bgra` to use presentation path (a) on macOS too, to compare the paths.
#[cfg(target_os = "macos")]
const PATH_ENV: &str = "RVLC_VIDEO_PATH";

#[cfg(target_os = "macos")]
fn native_sink(slot: &Arc<FrameSlot<Frame>>) -> Arc<dyn VideoSink> {
    if std::env::var(PATH_ENV).is_ok_and(|path| path == "bgra") {
        return Arc::new(bgra::BgraSink::new(slot.clone()));
    }
    Arc::new(pixel_buffer::PixelBufferSink::new(slot.clone()))
}

#[cfg(not(target_os = "macos"))]
fn native_sink(slot: &Arc<FrameSlot<Frame>>) -> Arc<dyn VideoSink> {
    Arc::new(bgra::BgraSink::new(slot.clone()))
}
