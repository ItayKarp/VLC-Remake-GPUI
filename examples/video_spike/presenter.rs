//! The GPUI side of each presentation path: takes the newest frame and builds its element.

use std::sync::Arc;
use std::time::Instant;

use futures::channel::mpsc::UnboundedReceiver;
use gpui::{
    AnyElement, ImageSource, IntoElement, ObjectFit, RenderImage, Styled, StyledImage as _, Window,
    div, img,
};

use crate::bgra::BgraSink;
use crate::options::PresentPath;
use crate::sink::{Size, VideoSink};
use crate::slot::FrameSlot;

/// The frame on screen for one presentation path.
pub enum Presenter {
    /// Path (a): `RenderImage`s in GPUI's atlas.
    Cpu { slot: Arc<FrameSlot<Arc<RenderImage>>>, shown: Option<Arc<RenderImage>> },
    /// Path (b): `CVPixelBuffer`s drawn by `surface()`.
    #[cfg(target_os = "macos")]
    Surface {
        slot: Arc<FrameSlot<crate::pixel_buffer::SendPixelBuffer>>,
        shown: Option<objc2_core_foundation::CFRetained<objc2_core_video::CVPixelBuffer>>,
    },
}

/// A presenter, the sink that feeds it, and the receiver that ticks when a frame arrives.
pub struct Pipeline {
    pub presenter: Presenter,
    pub sink: Arc<dyn VideoSink>,
    pub woken: UnboundedReceiver<()>,
}

impl Pipeline {
    /// Builds the pipeline for `path`, sizing pictures by `size`.
    pub fn new(path: PresentPath, size: Size) -> Self {
        match path {
            PresentPath::Cpu => {
                let (slot, woken) = FrameSlot::new();
                let slot = Arc::new(slot);
                let sink = Arc::new(BgraSink::new(slot.clone(), size));
                Self { presenter: Presenter::Cpu { slot, shown: None }, sink, woken }
            }
            #[cfg(target_os = "macos")]
            PresentPath::Surface => {
                let (slot, woken) = FrameSlot::new();
                let slot = Arc::new(slot);
                let sink = Arc::new(crate::pixel_buffer::PixelBufferSink::new(slot.clone(), size));
                Self { presenter: Presenter::Surface { slot, shown: None }, sink, woken }
            }
            #[cfg(not(target_os = "macos"))]
            PresentPath::Surface => unreachable!("options reject the surface path off macOS"),
        }
    }
}

impl Presenter {
    /// Swaps in the newest frame, if any, returning when libVLC said it was due.
    pub fn advance(&mut self, window: &mut Window) -> Option<Instant> {
        match self {
            Self::Cpu { slot, shown } => {
                let stamped = slot.take()?;
                // Each frame is a new atlas entry: free the old one or the atlas grows forever.
                if let Some(old) = shown.replace(stamped.frame) {
                    window.drop_image(old).ok();
                }
                Some(stamped.due)
            }
            #[cfg(target_os = "macos")]
            Self::Surface { slot, shown } => {
                let stamped = slot.take()?;
                *shown = Some(stamped.frame.0);
                Some(stamped.due)
            }
        }
    }

    /// The element showing the current frame, placed in its container by `fit`.
    pub fn element(&self, fit: ObjectFit) -> AnyElement {
        match self {
            Self::Cpu { shown: Some(image), .. } => img(ImageSource::Render(image.clone()))
                .size_full()
                .object_fit(fit)
                .into_any_element(),
            #[cfg(target_os = "macos")]
            Self::Surface { shown: Some(buffer), .. } => {
                gpui::surface(buffer.clone()).object_fit(fit).size_full().into_any_element()
            }
            _ => div().into_any_element(),
        }
    }

    /// `(published, replaced unseen)` frame counts of the slot.
    pub fn counts(&self) -> (u64, u64) {
        match self {
            Self::Cpu { slot, .. } => slot.counts(),
            #[cfg(target_os = "macos")]
            Self::Surface { slot, .. } => slot.counts(),
        }
    }
}

/// The largest box of `aspect` (width, height) that fits centred in `container`, as
/// `(left, top, width, height)`: the video area, with black bars around it.
pub fn letterbox(container: (f32, f32), aspect: (u32, u32)) -> (f32, f32, f32, f32) {
    let (container_width, container_height) = container;
    let ratio = aspect.0 as f32 / aspect.1.max(1) as f32;
    let (width, height) = if container_width / container_height > ratio {
        (container_height * ratio, container_height)
    } else {
        (container_width, container_width / ratio)
    };
    ((container_width - width) / 2.0, (container_height - height) / 2.0, width, height)
}

#[cfg(test)]
mod tests {
    use super::letterbox;

    /// `letterbox`, rounded to whole pixels.
    fn rounded(container: (f32, f32), aspect: (u32, u32)) -> [f32; 4] {
        let (left, top, width, height) = letterbox(container, aspect);
        [left, top, width, height].map(f32::round)
    }

    #[test]
    fn wide_video_gets_bars_above_and_below() {
        assert_eq!(rounded((1280.0, 720.0), (1920, 800)), [0.0, 93.0, 1280.0, 533.0]);
    }

    #[test]
    fn tall_video_gets_bars_at_the_sides() {
        assert_eq!(rounded((1280.0, 720.0), (1080, 1920)), [438.0, 0.0, 405.0, 720.0]);
    }

    #[test]
    fn anamorphic_dvd_fills_a_sixteen_by_nine_window() {
        assert_eq!(rounded((1280.0, 720.0), (853, 480)), [0.0, 0.0, 1280.0, 720.0]);
    }
}
