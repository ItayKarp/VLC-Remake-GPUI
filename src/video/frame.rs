//! A decoded picture in the form its presentation path draws, and how it is painted.

use std::sync::Arc;

use gpui::{Bounds, Corners, Pixels, RenderImage, Window};

/// One picture, ready for GPUI.
#[derive(Clone)]
pub enum Frame {
    /// Path (a): BGRA pixels that GPUI uploads into its texture atlas.
    Image(Arc<RenderImage>),
    /// Path (b), macOS: an IOSurface-backed NV12 buffer that GPUI samples without a copy.
    #[cfg(target_os = "macos")]
    Surface(objc2_core_foundation::CFRetained<objc2_core_video::CVPixelBuffer>),
}

// SAFETY: Core Video buffers are reference counted thread-safely, and the video thread never
// touches a buffer again after publishing it. `RenderImage` is plain owned bytes.
#[cfg(target_os = "macos")]
unsafe impl Send for Frame {}

impl Frame {
    /// The picture's size in pixels.
    pub fn size(&self) -> (u32, u32) {
        match self {
            Self::Image(image) => {
                let size = image.size(0);
                (size.width.0 as u32, size.height.0 as u32)
            }
            #[cfg(target_os = "macos")]
            Self::Surface(buffer) => (
                objc2_core_video::CVPixelBufferGetWidth(buffer) as u32,
                objc2_core_video::CVPixelBufferGetHeight(buffer) as u32,
            ),
        }
    }

    /// Paints the picture stretched over `bounds`.
    pub fn paint(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        match self {
            Self::Image(image) => {
                let painted =
                    window.paint_image(bounds, bounds, Corners::default(), image.clone(), 0, false);
                if let Err(error) = painted {
                    tracing::warn!("video: couldn't paint a frame: {error:#}");
                }
            }
            #[cfg(target_os = "macos")]
            Self::Surface(buffer) => window.paint_surface(bounds, buffer.clone()),
        }
    }

    /// Frees what GPUI keeps for this picture once it is replaced. Each BGRA frame is a new
    /// atlas entry, so without this the atlas grows forever (ADR 0001, limit 7).
    pub fn release(self, window: &mut Window) {
        if let Self::Image(image) = self {
            window.drop_image(image).ok();
        }
    }
}
