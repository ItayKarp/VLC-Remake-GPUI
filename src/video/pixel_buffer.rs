//! Presentation path (b), macOS (ADR 0001): the backend writes NV12 straight into
//! IOSurface-backed `CVPixelBuffer`s, which GPUI's surface primitive samples on the GPU without
//! another copy.

use std::ffi::c_void;
use std::ptr::{self, NonNull};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use anyhow::{Context as _, Result, ensure};
use objc2_core_foundation::{CFBoolean, CFDictionary, CFNumber, CFRetained, CFString, CFType};
use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferGetBaseAddressOfPlane, CVPixelBufferGetBytesPerRowOfPlane,
    CVPixelBufferGetHeightOfPlane, CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags,
    CVPixelBufferPool, CVPixelBufferUnlockBaseAddress, kCVPixelBufferHeightKey,
    kCVPixelBufferIOSurfacePropertiesKey, kCVPixelBufferMetalCompatibilityKey,
    kCVPixelBufferPixelFormatTypeKey, kCVPixelBufferWidthKey,
    kCVPixelFormatType_420YpCbCr8BiPlanarFullRange, kCVReturnSuccess,
};

use super::frame::Frame;
use super::frame_slot::FrameSlot;
use crate::engine::video::{Format, MAX_PLANES, VideoSink};

/// NV12 has a luma plane and an interleaved chroma plane.
const NV12_PLANES: usize = 2;

/// Turns pictures into `CVPixelBuffer`s.
pub struct PixelBufferSink {
    slot: Arc<FrameSlot<Frame>>,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    pool: Option<CFRetained<CVPixelBufferPool>>,
    /// The buffer being written, between `lock` and `display`.
    writing: Option<CFRetained<CVPixelBuffer>>,
}

// SAFETY: the state is only touched under its mutex, from the backend's video thread; Core
// Video objects are reference counted thread-safely.
unsafe impl Send for State {}

impl PixelBufferSink {
    /// A sink publishing into `slot`.
    pub fn new(slot: Arc<FrameSlot<Frame>>) -> Self {
        Self { slot, state: Mutex::default() }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl VideoSink for PixelBufferSink {
    fn setup(&self, format: &mut Format) -> bool {
        let (width, height) = format.chosen_size();
        let buffers = new_pool(width, height).and_then(|pool| Ok((new_buffer(&pool)?, pool)));
        let (probe, pool) = match buffers {
            Ok(buffers) => buffers,
            Err(error) => {
                tracing::warn!("video: no pixel buffers for {width}x{height}: {error:#}");
                return false;
            }
        };
        // The backend must write with the pool's row strides, which Core Video pads.
        for plane in 0..NV12_PLANES {
            format.pitches[plane] = CVPixelBufferGetBytesPerRowOfPlane(&probe, plane) as u32;
            format.lines[plane] = CVPixelBufferGetHeightOfPlane(&probe, plane) as u32;
        }
        format.chroma = *b"NV12";
        format.width = width;
        format.height = height;
        self.state().pool = Some(pool);
        true
    }

    fn lock(&self, planes: &mut [*mut c_void; MAX_PLANES]) {
        let mut state = self.state();
        let Some(buffer) = state.pool.as_ref().and_then(|pool| new_buffer(pool).ok()) else {
            return;
        };
        // SAFETY: the buffer is live; it is unlocked again in `unlock`.
        unsafe { CVPixelBufferLockBaseAddress(&buffer, CVPixelBufferLockFlags(0)) };
        for (plane, pointer) in planes.iter_mut().enumerate().take(NV12_PLANES) {
            *pointer = CVPixelBufferGetBaseAddressOfPlane(&buffer, plane);
        }
        state.writing = Some(buffer);
    }

    fn unlock(&self) {
        if let Some(buffer) = &self.state().writing {
            // SAFETY: `lock` locked this buffer with the same flags.
            unsafe { CVPixelBufferUnlockBaseAddress(buffer, CVPixelBufferLockFlags(0)) };
        }
    }

    fn display(&self) {
        // A refused buffer goes back to the pool when dropped here.
        let Some(buffer) = self.state().writing.take() else { return };
        self.slot.publish(Frame::Surface(buffer));
    }

    fn cleanup(&self) {
        self.state().writing = None;
        self.slot.clear();
    }
}

/// A pool of IOSurface-backed, Metal-compatible full-range NV12 buffers, the only format GPUI's
/// Metal renderer accepts for surfaces.
fn new_pool(width: u32, height: u32) -> Result<CFRetained<CVPixelBufferPool>> {
    let format = CFNumber::new_i32(kCVPixelFormatType_420YpCbCr8BiPlanarFullRange as i32);
    let width = CFNumber::new_i32(width as i32);
    let height = CFNumber::new_i32(height as i32);
    let io_surface = CFDictionary::<CFString, CFType>::empty();
    // SAFETY: these are Core Video's constant key strings.
    let keys: [&CFString; 5] = unsafe {
        [
            kCVPixelBufferPixelFormatTypeKey,
            kCVPixelBufferWidthKey,
            kCVPixelBufferHeightKey,
            kCVPixelBufferIOSurfacePropertiesKey,
            kCVPixelBufferMetalCompatibilityKey,
        ]
    };
    let values: [&CFType; 5] =
        [&format, &width, &height, &io_surface, CFBoolean::new(true).as_ref()];
    let attributes = CFDictionary::from_slices(&keys, &values);
    let mut pool = ptr::null_mut();
    // SAFETY: the attributes dictionary is live and `pool` is a writable out pointer.
    let status = unsafe {
        CVPixelBufferPool::create(None, None, Some(attributes.as_ref()), NonNull::from(&mut pool))
    };
    ensure!(status == kCVReturnSuccess, "CVPixelBufferPoolCreate failed: {status}");
    let pool = NonNull::new(pool).context("CVPixelBufferPoolCreate returned NULL")?;
    // SAFETY: a successful create hands over one reference.
    Ok(unsafe { CFRetained::from_raw(pool) })
}

fn new_buffer(pool: &CVPixelBufferPool) -> Result<CFRetained<CVPixelBuffer>> {
    let mut buffer = ptr::null_mut();
    // SAFETY: `pool` is live and `buffer` is a writable out pointer.
    let status =
        unsafe { CVPixelBufferPool::create_pixel_buffer(None, pool, NonNull::from(&mut buffer)) };
    ensure!(status == kCVReturnSuccess, "CVPixelBufferPoolCreatePixelBuffer failed: {status}");
    let buffer = NonNull::new(buffer).context("the pool returned NULL")?;
    // SAFETY: a successful create hands over one reference.
    Ok(unsafe { CFRetained::from_raw(buffer) })
}
