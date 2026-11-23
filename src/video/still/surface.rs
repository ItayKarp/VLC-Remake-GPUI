//! Reading an NV12 `CVPixelBuffer` back on the CPU (macOS).

use anyhow::{Result, ensure};
use image::RgbaImage;
use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferGetBaseAddressOfPlane, CVPixelBufferGetBytesPerRowOfPlane,
    CVPixelBufferGetHeight, CVPixelBufferGetHeightOfPlane, CVPixelBufferGetWidth,
    CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress,
    kCVReturnSuccess,
};

use super::nv12::{self, Planes};

/// `buffer`'s picture in RGBA. Fails when its memory can't be read (a protected surface).
pub fn read_back(buffer: &CVPixelBuffer) -> Result<RgbaImage> {
    let read_only = CVPixelBufferLockFlags::ReadOnly;
    // SAFETY: the buffer is live; it is unlocked below with the same flags.
    let status = unsafe { CVPixelBufferLockBaseAddress(buffer, read_only) };
    ensure!(status == kCVReturnSuccess, "the picture can't be read back ({status})");
    let picture = convert(buffer);
    // SAFETY: locked above with these flags.
    unsafe { CVPixelBufferUnlockBaseAddress(buffer, read_only) };
    picture
}

/// Converts the locked `buffer`.
fn convert(buffer: &CVPixelBuffer) -> Result<RgbaImage> {
    let plane = |index: usize| {
        let base = CVPixelBufferGetBaseAddressOfPlane(buffer, index).cast::<u8>().cast_const();
        let stride = CVPixelBufferGetBytesPerRowOfPlane(buffer, index);
        let rows = CVPixelBufferGetHeightOfPlane(buffer, index);
        (base, stride, rows)
    };
    let ((luma, luma_stride, luma_rows), (chroma, chroma_stride, chroma_rows)) =
        (plane(0), plane(1));
    ensure!(!luma.is_null() && !chroma.is_null(), "the picture can't be read back");
    // SAFETY: the buffer is locked, so each plane's `stride * rows` bytes are readable until it
    // is unlocked, after this function returns.
    let (luma, chroma) = unsafe {
        (
            std::slice::from_raw_parts(luma, luma_stride * luma_rows),
            std::slice::from_raw_parts(chroma, chroma_stride * chroma_rows),
        )
    };
    let (width, height) = (CVPixelBufferGetWidth(buffer), CVPixelBufferGetHeight(buffer));
    let planes = Planes { luma, luma_stride, chroma, chroma_stride };
    Ok(nv12::to_rgba(&planes, width as u32, height as u32))
}
