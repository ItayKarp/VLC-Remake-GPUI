//! Seek previews (S2.3.3): pictures of the media at a time the user points at on the seek bar,
//! from a **second, muted decoder**, so the main player is never seeked to build them.
//!
//! - [`PreviewDecoder`] is the interface: open the media, ask for times, get [`PreviewEvent`]s
//!   back through the channel it was built with.
//! - [`flow`] is the decoder-independent request logic: one request at a time, the newest
//!   waiting request replacing older ones, and timeouts so a slow or broken file never wedges
//!   it.
//! - The libVLC implementation lives in `libvlc::preview`; [`default_decoder`] falls back to one
//!   that answers every request with "no picture" when libVLC is missing.

pub mod flow;

use std::time::Duration;

use futures::channel::mpsc;

use crate::app::media::Media;

/// The largest picture a preview asks for: the hover card's frame at 2× scale.
pub const MAX_WIDTH: u32 = 480;
/// The largest preview height.
pub const MAX_HEIGHT: u32 = 270;

/// A picture for one request: BGRA pixels, `width × height`, rows packed.
#[derive(Clone, PartialEq, Eq)]
pub struct PreviewImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes, blue first.
    pub bgra: Vec<u8>,
}

impl std::fmt::Debug for PreviewImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PreviewImage({}x{})", self.width, self.height)
    }
}

/// A picture wanted at `time`, named by `id` so answers to stale requests can be told apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreviewRequest {
    /// Increases with every request the UI makes.
    pub id: u64,
    /// Where in the media.
    pub time: Duration,
}

/// The decoder's answer to a request. Requests that a newer one replaced before they started
/// are never answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewEvent {
    /// The request answered.
    pub id: u64,
    /// The picture, or `None` when none could be made (no video, a damaged region, a timeout).
    pub image: Option<PreviewImage>,
}

/// Sends answers to the UI; safe to use from any thread.
#[derive(Clone, Debug)]
pub struct PreviewSender(mpsc::UnboundedSender<PreviewEvent>);

impl PreviewSender {
    /// Queues `event` for the UI. Does nothing once the UI is gone.
    pub fn send(&self, event: PreviewEvent) {
        // An error only means the receiver was dropped; nobody is left to tell.
        let _ = self.0.unbounded_send(event);
    }
}

/// The UI's end of the answer channel.
pub type PreviewReceiver = mpsc::UnboundedReceiver<PreviewEvent>;

/// A new answer channel.
pub fn channel() -> (PreviewSender, PreviewReceiver) {
    let (sender, receiver) = mpsc::unbounded();
    (PreviewSender(sender), receiver)
}

/// A decoder of preview pictures. Every method returns at once; the work happens elsewhere.
pub trait PreviewDecoder: Send {
    /// Opens `media` for previews, forgetting whatever was open and its waiting requests.
    fn open(&mut self, media: &Media);
    /// Asks for a picture. A request waiting behind a running one is replaced by this one.
    fn request(&mut self, request: PreviewRequest);
    /// Closes the media, releasing its decoder.
    fn close(&mut self);
}

/// The preview decoder to use: libVLC, or one without pictures when libVLC can't be loaded.
pub fn default_decoder(events: PreviewSender) -> Box<dyn PreviewDecoder> {
    match super::libvlc::preview::LibVlcPreview::new(events.clone()) {
        Ok(decoder) => Box::new(decoder),
        Err(error) => {
            tracing::warn!("preview: no preview decoder: {error:#}");
            Box::new(NoPictures(events))
        }
    }
}

/// Answers every request without a picture: the hover card then shows only the time.
pub struct NoPictures(pub PreviewSender);

impl PreviewDecoder for NoPictures {
    fn open(&mut self, _: &Media) {}

    fn request(&mut self, request: PreviewRequest) {
        self.0.send(PreviewEvent { id: request.id, image: None });
    }

    fn close(&mut self) {}
}

/// `width × height` shrunk to fit [`MAX_WIDTH`] × [`MAX_HEIGHT`], keeping the aspect ratio and
/// even sides (the scaler's chroma needs them). Never zero.
pub fn fit(width: u32, height: u32) -> (u32, u32) {
    fit_within(width, height, (MAX_WIDTH, MAX_HEIGHT))
}

/// `width × height` shrunk to fit `bounds` (width, height), as [`fit`] does for the preview's.
pub fn fit_within(width: u32, height: u32, bounds: (u32, u32)) -> (u32, u32) {
    let (width, height) = (u64::from(width.max(1)), u64::from(height.max(1)));
    let (max_width, max_height) = (u64::from(bounds.0), u64::from(bounds.1));
    // Scale by whichever side is tighter: width / max_width against height / max_height.
    let (scaled_width, scaled_height) = if width * max_height >= height * max_width {
        (max_width, height * max_width / width)
    } else {
        (width * max_height / height, max_height)
    };
    let even = |side: u64| ((side as u32) & !1).max(2);
    (even(scaled_width.min(width)), even(scaled_height.min(height)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_video_fits_the_width() {
        assert_eq!(fit(1920, 1080), (480, 270));
        assert_eq!(fit(1920, 800), (480, 200));
    }

    #[test]
    fn tall_video_fits_the_height() {
        assert_eq!(fit(1080, 1920), (150, 270));
    }

    #[test]
    fn small_video_is_not_enlarged_and_sizes_stay_even() {
        assert_eq!(fit(321, 181), (320, 180));
        assert_eq!(fit(0, 0), (2, 2));
    }

    #[test]
    fn no_pictures_answers_every_request() {
        let (sender, mut receiver) = channel();
        let mut decoder = NoPictures(sender);
        decoder.request(PreviewRequest { id: 7, time: Duration::from_secs(1) });
        assert_eq!(receiver.try_recv().ok(), Some(PreviewEvent { id: 7, image: None }));
    }
}
