//! The hover card's pictures (S2.3.3): asks the preview decoder for the time under the
//! pointer and keeps the newest answer as an image GPUI can draw.
//!
//! The decoder is created on the first hover over video, so playing audio, or never hovering,
//! never starts a second libVLC. Times are rounded to a step of the media's length, so moving
//! within one step asks nothing new; the decoder itself runs one request at a time and keeps
//! only the newest waiting one.

use std::sync::Arc;
use std::time::Duration;

use gpui::{RenderImage, Window};
use image::{Frame, RgbaImage};

use crate::app::media::Media;
use crate::engine::preview::{PreviewDecoder, PreviewEvent, PreviewImage, PreviewRequest};

/// The track is split into this many preview steps at most.
const STEPS: u32 = 400;
/// The finest preview step.
const MIN_STEP: Duration = Duration::from_millis(100);

/// What the card shows for the newest answered request.
pub enum Picture {
    /// A frame.
    Frame(Arc<RenderImage>),
    /// No frame could be made: a grey placeholder (a damaged region, or no decoder).
    Missing,
}

/// The preview decoder and its answers, for the window's seek bar.
#[derive(Default)]
pub struct Previews {
    decoder: Option<Box<dyn PreviewDecoder>>,
    /// Whether the decoder is being created, off the UI thread.
    creating: bool,
    /// What the decoder has open.
    media: Option<Media>,
    next_id: u64,
    /// The step last asked for.
    asked: Option<Duration>,
    /// The newest answer: its request id and picture.
    shown: Option<(u64, Picture)>,
    /// Replaced frames, to free from GPUI's atlas when the window next draws.
    retired: Vec<Arc<RenderImage>>,
}

impl Previews {
    /// Whether a decoder should be created: there is none, and none on its way. Answers `true`
    /// once, then waits for [`Self::set_decoder`].
    pub fn needs_decoder(&mut self) -> bool {
        let needed = self.decoder.is_none() && !self.creating;
        self.creating |= needed;
        needed
    }

    /// Takes the decoder to ask from now on.
    pub fn set_decoder(&mut self, decoder: Box<dyn PreviewDecoder>) {
        self.decoder = Some(decoder);
        self.creating = false;
    }

    /// Asks for a picture of `media`, `duration` long, at `time`, unless the same step was
    /// asked last.
    pub fn want(&mut self, media: &Media, time: Duration, duration: Duration) {
        if self.decoder.is_none() {
            return;
        }
        if self.media.as_ref() != Some(media) {
            self.retire_shown();
            self.media = Some(media.clone());
            self.asked = None;
            if let Some(decoder) = self.decoder.as_deref_mut() {
                decoder.open(media);
            }
        }
        let step = (duration / STEPS).max(MIN_STEP);
        let time = step * u32::try_from(time.as_nanos() / step.as_nanos()).unwrap_or(u32::MAX);
        if self.asked == Some(time) {
            return;
        }
        self.asked = Some(time);
        self.next_id += 1;
        if let Some(decoder) = self.decoder.as_deref_mut() {
            decoder.request(PreviewRequest { id: self.next_id, time });
        }
    }

    /// Takes an answer. Older answers than the one shown are dropped.
    pub fn answered(&mut self, event: PreviewEvent) {
        if self.shown.as_ref().is_some_and(|(id, _)| *id >= event.id) || event.id > self.next_id {
            return;
        }
        let picture = event.image.and_then(render_image).map_or(Picture::Missing, Picture::Frame);
        self.retire_shown();
        self.shown = Some((event.id, picture));
    }

    /// The newest picture, if any request was answered for the open media.
    pub fn picture(&self) -> Option<&Picture> {
        self.shown.as_ref().map(|(_, picture)| picture)
    }

    /// Closes the media: a new open is coming, or nothing plays.
    pub fn forget(&mut self) {
        if let Some(decoder) = self.decoder.as_deref_mut()
            && self.media.take().is_some()
        {
            decoder.close();
        }
        self.asked = None;
        self.retire_shown();
    }

    /// Frees the replaced frames from GPUI's atlas.
    pub fn release_retired(&mut self, window: &mut Window) {
        for image in self.retired.drain(..) {
            window.drop_image(image).ok();
        }
    }

    fn retire_shown(&mut self) {
        if let Some((_, Picture::Frame(image))) = self.shown.take() {
            self.retired.push(image);
        }
    }
}

/// `image` as a GPUI image; its `RgbaImage` container holds BGRA bytes, as GPUI expects.
fn render_image(image: PreviewImage) -> Option<Arc<RenderImage>> {
    let pixels = RgbaImage::from_raw(image.width, image.height, image.bgra)?;
    Some(Arc::new(RenderImage::new([Frame::new(pixels)])))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use super::*;

    /// Records what the decoder was asked.
    #[derive(Clone, Default)]
    struct Recorder(Arc<Mutex<Vec<String>>>);

    impl PreviewDecoder for Recorder {
        fn open(&mut self, media: &Media) {
            self.0.lock().unwrap().push(format!("open {}", media.display_name()));
        }

        fn request(&mut self, request: PreviewRequest) {
            self.0.lock().unwrap().push(format!("{} at {:?}", request.id, request.time));
        }

        fn close(&mut self) {
            self.0.lock().unwrap().push("close".into());
        }
    }

    fn previews() -> (Previews, Recorder) {
        let recorder = Recorder::default();
        let mut previews = Previews::default();
        previews.set_decoder(Box::new(recorder.clone()));
        (previews, recorder)
    }

    fn media(name: &str) -> Media {
        Media::File(PathBuf::from(format!("/videos/{name}")))
    }

    fn image() -> PreviewImage {
        PreviewImage { width: 2, height: 2, bgra: vec![0; 16] }
    }

    #[test]
    fn opens_the_media_once_and_skips_repeated_steps() {
        let (mut previews, recorder) = previews();
        let hour = Duration::from_secs(3600);
        previews.want(&media("a.mkv"), Duration::from_secs(90), hour);
        previews.want(&media("a.mkv"), Duration::from_secs(91), hour);
        previews.want(&media("a.mkv"), Duration::from_secs(99), hour);
        assert_eq!(*recorder.0.lock().unwrap(), ["open a.mkv", "1 at 90s", "2 at 99s"]);
    }

    #[test]
    fn new_media_is_opened_and_forgetting_closes_it() {
        let (mut previews, recorder) = previews();
        let length = Duration::from_secs(10);
        previews.want(&media("a.mkv"), Duration::ZERO, length);
        previews.want(&media("b.mkv"), Duration::ZERO, length);
        previews.forget();
        let asked = recorder.0.lock().unwrap().clone();
        assert_eq!(asked, ["open a.mkv", "1 at 0ns", "open b.mkv", "2 at 0ns", "close"]);
    }

    #[test]
    fn stale_answers_are_dropped_and_failures_show_a_placeholder() {
        let (mut previews, _) = previews();
        let length = Duration::from_secs(10);
        for secs in 1..=3 {
            previews.want(&media("a.mkv"), Duration::from_secs(secs), length);
        }
        previews.answered(PreviewEvent { id: 2, image: Some(image()) });
        previews.answered(PreviewEvent { id: 1, image: Some(image()) });
        assert!(matches!(previews.picture(), Some(Picture::Frame(_))));
        previews.answered(PreviewEvent { id: 3, image: None });
        assert!(matches!(previews.picture(), Some(Picture::Missing)));
        assert_eq!(previews.retired.len(), 1, "the replaced frame is freed later");
    }
}
