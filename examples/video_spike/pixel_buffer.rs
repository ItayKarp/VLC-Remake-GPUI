//! Presentation path (b), macOS: libVLC writes NV12 straight into IOSurface-backed
//! `CVPixelBuffer`s, which GPUI's `surface()` element samples on the GPU without another copy.
//!
//! The buffers are video range ('420v'), which is what libVLC writes, and are tagged with
//! their YCbCr matrix, so GPUI's surface shader (patched in our Zed fork) converts them with
//! the right colors.

use std::ffi::c_void;
use std::ptr::{self, NonNull};
use std::sync::{Arc, Mutex};

use anyhow::{Context as _, Result, ensure};
use objc2_core_foundation::{CFBoolean, CFDictionary, CFNumber, CFRetained, CFString, CFType};
use objc2_core_video::{
    CVAttachmentMode, CVPixelBuffer, CVPixelBufferGetBaseAddressOfPlane,
    CVPixelBufferGetBytesPerRowOfPlane, CVPixelBufferGetHeightOfPlane,
    CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags, CVPixelBufferPool,
    CVPixelBufferUnlockBaseAddress, kCVImageBufferYCbCrMatrix_ITU_R_601_4,
    kCVImageBufferYCbCrMatrix_ITU_R_709_2, kCVImageBufferYCbCrMatrixKey, kCVPixelBufferHeightKey,
    kCVPixelBufferIOSurfacePropertiesKey, kCVPixelBufferMetalCompatibilityKey,
    kCVPixelBufferPixelFormatTypeKey, kCVPixelBufferWidthKey,
    kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange, kCVReturnSuccess,
};

use crate::sink::{Format, MAX_PLANES, Size, VideoSink, choose_size};
use crate::slot::FrameSlot;

/// NV12 has a luma plane and an interleaved chroma plane.
const NV12_PLANES: usize = 2;
/// The largest standard-definition picture. libVLC 3 does not report a track's YCbCr matrix,
/// so, like other players, we take larger pictures to be BT.709 and the rest BT.601.
const LARGEST_SD: (u32, u32) = (1024, 576);

/// A pixel buffer that may cross to the GPUI thread.
pub struct SendPixelBuffer(pub CFRetained<CVPixelBuffer>);

// SAFETY: Core Video buffers are reference counted thread-safely, and the video thread never
// touches a buffer again after publishing it.
unsafe impl Send for SendPixelBuffer {}

/// Turns libVLC pictures into `CVPixelBuffer`s for `gpui::surface()`.
pub struct PixelBufferSink {
    slot: Arc<FrameSlot<SendPixelBuffer>>,
    size: Size,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    pool: Option<CFRetained<CVPixelBufferPool>>,
    writing: Option<CFRetained<CVPixelBuffer>>,
    /// Whether pictures are HD, and so BT.709 rather than BT.601.
    hd: bool,
}

// SAFETY: the state is only touched under its mutex, from libVLC's video thread.
unsafe impl Send for State {}

impl PixelBufferSink {
    /// A sink publishing into `slot`, sizing pictures by `size`.
    pub fn new(slot: Arc<FrameSlot<SendPixelBuffer>>, size: Size) -> Self {
        Self { slot, size, state: Mutex::default() }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().expect("pixel buffer sink lock poisoned")
    }
}

impl VideoSink for PixelBufferSink {
    fn setup(&self, format: &mut Format) -> bool {
        let (width, height) = choose_size(format, self.size.display, self.size.max_texture);
        let Ok(pool) = new_pool(width, height) else { return false };
        let Ok(probe) = new_buffer(&pool) else { return false };
        // libVLC must write with the pool's row strides, which Core Video pads.
        for plane in 0..NV12_PLANES {
            format.pitches[plane] = CVPixelBufferGetBytesPerRowOfPlane(&probe, plane) as u32;
            format.lines[plane] = CVPixelBufferGetHeightOfPlane(&probe, plane) as u32;
        }
        format.chroma = *b"NV12";
        format.width = width;
        format.height = height;
        let mut state = self.state();
        state.pool = Some(pool);
        state.hd = is_hd(width, height);
        true
    }

    fn lock(&self, planes: &mut [*mut c_void; MAX_PLANES]) {
        let mut state = self.state();
        let Some(buffer) = state.pool.as_ref().and_then(|pool| new_buffer(pool).ok()) else {
            return;
        };
        tag_matrix(&buffer, state.hd);
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
        let Some(buffer) = self.state().writing.take() else { return };
        self.slot.publish(SendPixelBuffer(buffer));
    }
}

/// Whether a `width`×`height` picture counts as HD (see [`LARGEST_SD`]).
fn is_hd(width: u32, height: u32) -> bool {
    width > LARGEST_SD.0 || height > LARGEST_SD.1
}

/// Tags `buffer` with BT.709 when `hd`, else BT.601, for GPUI's surface shader.
fn tag_matrix(buffer: &CVPixelBuffer, hd: bool) {
    // SAFETY: the key and values are Core Video's constant strings; the buffer is live.
    unsafe {
        let matrix = if hd {
            kCVImageBufferYCbCrMatrix_ITU_R_709_2
        } else {
            kCVImageBufferYCbCrMatrix_ITU_R_601_4
        };
        buffer.set_attachment(
            kCVImageBufferYCbCrMatrixKey,
            matrix.as_ref(),
            CVAttachmentMode::ShouldPropagate,
        );
    }
}

/// A pool of IOSurface-backed, Metal-compatible video-range NV12 buffers.
fn new_pool(width: u32, height: u32) -> Result<CFRetained<CVPixelBufferPool>> {
    let format = CFNumber::new_i32(kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange as i32);
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

#[cfg(test)]
mod tests {
    use super::is_hd;

    #[test]
    fn hd_starts_above_standard_definition() {
        assert!(!is_hd(720, 480));
        assert!(!is_hd(1024, 576));
        assert!(is_hd(1280, 720));
        assert!(is_hd(1080, 1920));
    }
}
