//! A still of the picture on screen, for snapshots (S8.4): the frame the view shows, read back
//! into RGBA at the size it was decoded at, then framed as the screen frames it.
//!
//! What the frame holds is what libVLC drew into it: the video filters, the flip and rotation,
//! HDR tone mapping (S7.5) and the subtitles libVLC renders are all in it, as in VLC's own
//! snapshots. The aspect ratio, trimmed edges and crop are the view's, so they are applied here;
//! the zoom only sizes the picture on screen and is left out. Subtitle files the app draws itself
//! (S6.2) are not in it.

#[cfg(any(target_os = "macos", test))]
mod nv12;
#[cfg(target_os = "macos")]
mod surface;

use anyhow::{Context as _, Result};
use image::RgbaImage;
use image::imageops::{self, FilterType};

use super::frame::Frame;
use super::framing::{self, Framing, Part, Source};

/// The picture on screen and how it is framed, ready to read back off the UI thread.
pub struct Still {
    frame: Frame,
    source: Source,
    framing: Framing,
}

impl Still {
    pub(super) fn new(frame: Frame, source: Source, framing: Framing) -> Self {
        Self { frame, source, framing }
    }

    /// The picture in RGBA, framed as the screen shows it. Slow for a large picture: call it
    /// off the UI thread. Fails when the picture can't be read back (a protected surface).
    pub fn picture(&self) -> Result<RgbaImage> {
        let whole = read_back(&self.frame)?;
        Ok(cut(&whole, framing::part(self.source, self.framing)))
    }
}

/// The frame's pixels in RGBA.
fn read_back(frame: &Frame) -> Result<RgbaImage> {
    match frame {
        Frame::Image(image) => {
            let size = image.size(0);
            let bytes = image.as_bytes(0).context("the picture has no pixels")?;
            let mut pixels = bytes.to_vec();
            // GPUI keeps BGRA in its RGBA container.
            pixels.as_chunks_mut::<4>().0.iter_mut().for_each(|pixel| pixel.swap(0, 2));
            let (width, height) = (size.width.0 as u32, size.height.0 as u32);
            RgbaImage::from_raw(width, height, pixels).context("the picture's size is wrong")
        }
        #[cfg(target_os = "macos")]
        Frame::Surface(buffer) => surface::read_back(buffer),
    }
}

/// The `part` of `whole` that shows, stretched to the part's shape: its height in pixels is
/// kept and its width follows the shape.
fn cut(whole: &RgbaImage, part: Part) -> RgbaImage {
    let (width, height) = (whole.width() as f32, whole.height() as f32);
    let left = (part.left * width).round().clamp(0., width - 1.) as u32;
    let top = (part.top * height).round().clamp(0., height - 1.) as u32;
    let kept_width = ((part.width * width).round() as u32).clamp(1, whole.width() - left);
    let kept_height = ((part.height * height).round() as u32).clamp(1, whole.height() - top);
    let kept = imageops::crop_imm(whole, left, top, kept_width, kept_height).to_image();
    let shaped_width = ((kept_height as f32 * part.ratio).round() as u32).max(1);
    if shaped_width.abs_diff(kept_width) <= 1 {
        return kept;
    }
    imageops::resize(&kept, shaped_width, kept_height, FilterType::CatmullRom)
}

#[cfg(test)]
mod tests {
    use image::Rgba;

    use super::*;

    fn part(left: f32, top: f32, width: f32, height: f32, ratio: f32) -> Part {
        Part { left, top, width, height, ratio }
    }

    #[test]
    fn the_whole_picture_comes_back_as_it_is() {
        let whole = RgbaImage::from_pixel(1920, 1080, Rgba([1, 2, 3, 255]));
        let cut = cut(&whole, part(0., 0., 1., 1., 1920. / 1080.));
        assert_eq!(cut.dimensions(), (1920, 1080));
        assert_eq!(cut.get_pixel(5, 5), &Rgba([1, 2, 3, 255]));
    }

    #[test]
    fn a_crop_keeps_its_middle() {
        let mut whole = RgbaImage::new(400, 100);
        whole.put_pixel(150, 50, Rgba([255, 0, 0, 255]));
        let cut = cut(&whole, part(0.25, 0., 0.5, 1., 2.));
        assert_eq!(cut.dimensions(), (200, 100));
        assert_eq!(cut.get_pixel(50, 50), &Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn a_forced_aspect_ratio_stretches_the_width() {
        let whole = RgbaImage::new(720, 576);
        assert_eq!(cut(&whole, part(0., 0., 1., 1., 16. / 9.)).dimensions(), (1024, 576));
    }
}
