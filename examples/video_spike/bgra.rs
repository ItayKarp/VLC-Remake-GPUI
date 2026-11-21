//! Presentation path (a): libVLC hands over NV12 pictures in CPU memory, we convert them to
//! BGRA, and each frame becomes a GPUI `RenderImage` that GPUI uploads into its texture
//! atlas. Works on every OS.
//!
//! libVLC 3 can convert to BGRA itself, but always with the BT.601 matrix, which shifts the
//! colours of HD video. Converting ourselves lets us pick the matrix (see [`crate::colour`]).

use std::ffi::c_void;
use std::sync::{Arc, Mutex};

use gpui::RenderImage;
use image::{Frame, RgbaImage};
use yuvutils_rs::{
    YuvBiPlanarImage, YuvConversionMode, YuvRange, YuvStandardMatrix, yuv_nv12_to_bgra,
};

use crate::colour::is_hd;
use crate::sink::{Format, MAX_PLANES, Size, VideoSink, choose_size};
use crate::slot::FrameSlot;

const BYTES_PER_PIXEL: u32 = 4;

/// Turns libVLC pictures into `RenderImage`s.
pub struct BgraSink {
    slot: Arc<FrameSlot<Arc<RenderImage>>>,
    size: Size,
    picture: Mutex<Option<Picture>>,
}

/// The agreed picture format and the NV12 buffer libVLC writes every picture into. libVLC
/// calls `lock`, `unlock` and `display` for one picture before it locks the next, so one
/// buffer, reused, is enough.
struct Picture {
    layout: Nv12Layout,
    matrix: YuvStandardMatrix,
    nv12: Vec<u8>,
}

impl BgraSink {
    /// A sink publishing into `slot`, sizing pictures by `size`.
    pub fn new(slot: Arc<FrameSlot<Arc<RenderImage>>>, size: Size) -> Self {
        Self { slot, size, picture: Mutex::default() }
    }

    fn picture(&self) -> std::sync::MutexGuard<'_, Option<Picture>> {
        self.picture.lock().expect("BGRA sink lock poisoned")
    }
}

impl VideoSink for BgraSink {
    fn setup(&self, format: &mut Format) -> bool {
        let (width, height) = choose_size(format, self.size.display, self.size.max_texture);
        let layout = Nv12Layout::new(width, height);
        format.chroma = *b"NV12";
        format.width = width;
        format.height = height;
        format.pitches[..2].fill(layout.stride);
        format.lines[0] = height;
        format.lines[1] = layout.chroma_rows;
        let matrix =
            if is_hd(width, height) { YuvStandardMatrix::Bt709 } else { YuvStandardMatrix::Bt601 };
        let nv12 = vec![0; layout.len()];
        *self.picture() = Some(Picture { layout, matrix, nv12 });
        true
    }

    fn lock(&self, planes: &mut [*mut c_void; MAX_PLANES]) {
        let mut picture = self.picture();
        let Some(Picture { layout, nv12, .. }) = picture.as_mut() else { return };
        let (luma, chroma) = nv12.split_at_mut(layout.luma_len());
        planes[0] = luma.as_mut_ptr().cast();
        planes[1] = chroma.as_mut_ptr().cast();
    }

    fn unlock(&self) {}

    fn display(&self) {
        let guard = self.picture();
        let Some(image) = guard.as_ref().and_then(Picture::to_bgra) else { return };
        drop(guard);
        self.slot.publish(Arc::new(RenderImage::new([Frame::new(image)])));
    }
}

impl Picture {
    /// The picture as BGRA, in the `RgbaImage` container GPUI's `RenderImage` expects. A fresh
    /// buffer per frame: the `RenderImage` takes ownership of it, so nothing can be recycled.
    fn to_bgra(&self) -> Option<RgbaImage> {
        let Nv12Layout { width, height, stride, .. } = self.layout;
        let (luma, chroma) = self.nv12.split_at(self.layout.luma_len());
        let source = YuvBiPlanarImage {
            y_plane: luma,
            y_stride: stride,
            uv_plane: chroma,
            uv_stride: stride,
            width,
            height,
        };
        let mut bgra = vec![0; (width * height * BYTES_PER_PIXEL) as usize];
        // libVLC writes video ("limited") range, as decoders output it.
        yuv_nv12_to_bgra(
            &source,
            &mut bgra,
            width * BYTES_PER_PIXEL,
            YuvRange::Limited,
            self.matrix,
            YuvConversionMode::Balanced,
        )
        .ok()?;
        RgbaImage::from_raw(width, height, bgra)
    }
}

/// Where NV12's planes sit in one buffer: full-size luma, then half-height chroma with
/// interleaved Cb and Cr, both with the same row stride.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Nv12Layout {
    width: u32,
    height: u32,
    /// Bytes per row of both planes. A chroma row holds a Cb and Cr pair for every two
    /// pixels, rounded up, so odd widths need one extra byte.
    stride: u32,
    /// Rows of the chroma plane: one per two pixel rows, rounded up.
    chroma_rows: u32,
}

impl Nv12Layout {
    fn new(width: u32, height: u32) -> Self {
        Self { width, height, stride: width.next_multiple_of(2), chroma_rows: height.div_ceil(2) }
    }

    fn luma_len(self) -> usize {
        (self.stride * self.height) as usize
    }

    fn len(self) -> usize {
        self.luma_len() + (self.stride * self.chroma_rows) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::Nv12Layout;

    #[test]
    fn even_sizes_pack_without_padding() {
        let layout = Nv12Layout::new(1920, 1080);
        assert_eq!((layout.stride, layout.chroma_rows), (1920, 540));
        assert_eq!(layout.len(), 1920 * 1080 * 3 / 2);
    }

    #[test]
    fn odd_sizes_round_chroma_up() {
        let layout = Nv12Layout::new(1919, 1079);
        assert_eq!((layout.stride, layout.chroma_rows), (1920, 540));
        assert_eq!(layout.luma_len(), 1920 * 1079);
        assert_eq!(layout.len(), 1920 * 1079 + 1920 * 540);
    }
}
