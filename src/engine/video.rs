//! Where the engine delivers decoded pictures (S1.4.1): the [`VideoSink`] interface a
//! presentation path implements, and the picture format the backend and the sink agree on.
//!
//! The engine only knows this interface. The GPUI side (`crate::video`) implements it with the
//! presentation paths ADR 0001 chose: IOSurface-backed pixel buffers on macOS, BGRA images
//! elsewhere.

use std::ffi::c_void;

use super::media_info::VideoSize;

/// The most planes a picture has: libVLC's `PICTURE_PLANE_MAX`.
pub const MAX_PLANES: usize = 5;

/// The largest texture side a sink asks for. GPUI's atlas and Metal both accept 8192 px;
/// bigger video (16K) is shrunk by the backend's scaler.
pub const MAX_TEXTURE_SIDE: u32 = 8192;

/// The picture format the backend proposes, and the sink's answer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Format {
    /// A VLC fourcc such as `BGRA` or `NV12`.
    pub chroma: [u8; 4],
    /// Picture width in pixels; the backend scales to whatever the sink answers.
    pub width: u32,
    /// Picture height in pixels.
    pub height: u32,
    /// Bytes per row of each plane.
    pub pitches: [u32; MAX_PLANES],
    /// Rows of each plane.
    pub lines: [u32; MAX_PLANES],
    /// The track's display size (square pixels, after rotation) when the backend read it
    /// before playback; the proposed size is then the decoder's coded size, padding included.
    pub display: Option<VideoSize>,
}

impl Format {
    /// The size a sink should ask for: the display size when known (the backend then crops
    /// coded padding and squares anamorphic pixels for free), else the proposal, shrunk to fit
    /// [`MAX_TEXTURE_SIDE`].
    pub fn chosen_size(&self) -> (u32, u32) {
        let (width, height) =
            self.display.map_or((self.width, self.height), |size| (size.width, size.height));
        fit_texture(width, height, MAX_TEXTURE_SIDE)
    }
}

/// Receives decoded pictures. Every method runs on the backend's video thread, one picture at a
/// time: `lock`, the backend writes the picture, `unlock`, then `display` when it is due.
pub trait VideoSink: Send + Sync {
    /// Agrees on the format, editing `format` in place; `false` refuses the video.
    fn setup(&self, format: &mut Format) -> bool;
    /// Fills `planes` with buffers for the next picture.
    fn lock(&self, planes: &mut [*mut c_void; MAX_PLANES]);
    /// The backend finished writing the locked picture.
    fn unlock(&self);
    /// The last unlocked picture is due on screen.
    fn display(&self);
    /// The video output closed (stop, end, a new media, or a track change): nothing shows now.
    fn cleanup(&self);
}

/// Shrinks `width`×`height` to fit `max` on both sides, keeping the aspect ratio and even
/// sizes (4:2:0 chroma needs them). Sizes that already fit come back unchanged.
pub fn fit_texture(width: u32, height: u32, max: u32) -> (u32, u32) {
    if width <= max && height <= max {
        return (width, height);
    }
    let longest = u64::from(width.max(height));
    let even = |side: u32| ((u64::from(side) * u64::from(max) / longest) as u32 & !1).max(2);
    (even(width), even(height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_that_fit_are_unchanged() {
        assert_eq!(fit_texture(1919, 1079, 8192), (1919, 1079));
        assert_eq!(fit_texture(8192, 4320, 8192), (8192, 4320));
    }

    #[test]
    fn oversized_video_shrinks_to_the_limit_keeping_aspect() {
        assert_eq!(fit_texture(15360, 8640, 8192), (8192, 4608));
        assert_eq!(fit_texture(4320, 7680, 4096), (2304, 4096));
    }

    #[test]
    fn shrunk_sizes_are_even() {
        let (width, height) = fit_texture(16385, 9999, 8191);
        assert_eq!((width % 2, height % 2), (0, 0));
    }

    #[test]
    fn the_display_size_wins_over_the_coded_size() {
        let coded = Format { width: 1920, height: 1088, ..Format::default() };
        assert_eq!(coded.chosen_size(), (1920, 1088));
        let display = Some(VideoSize { width: 1920, height: 1080 });
        assert_eq!(Format { display, ..coded }.chosen_size(), (1920, 1080));
    }
}
