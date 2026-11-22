//! Making one media's pictures: a frame of a video, grabbed at the times [`pick`] chooses, or a
//! song's cover, cut to every size the cache keeps.

use std::path::Path;
use std::time::Duration;

use image::DynamicImage;

use super::pick::{self, Best};
use crate::db::thumbnails::Cached;
use crate::engine::thumbnail::{FrameGrabber, Grab};
use crate::library::artwork_cache::{ArtworkCache, MAX_SOURCE_BYTES};
use crate::library::thumbnail_cache::{self, Source, ThumbnailCache};

/// How making a media's pictures went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Made {
    /// Saved in every size.
    Pictures(Vec<Cached>),
    /// It has none: no picture could be grabbed, or a song has no cover.
    Nothing,
    /// Cut short: no longer wanted, or there is nothing to grab with. Try again later.
    Stopped,
}

/// Makes pictures with a grabber, into the cache.
pub struct Maker {
    grabber: Box<dyn FrameGrabber>,
    cache: ThumbnailCache,
    covers: Option<ArtworkCache>,
    /// The grabber said it has no backend: videos aren't tried again this run.
    unavailable: bool,
}

impl Maker {
    /// Grabs with `grabber` into `cache`, reading songs' covers from `covers`.
    pub fn new(
        grabber: Box<dyn FrameGrabber>,
        cache: ThumbnailCache,
        covers: Option<ArtworkCache>,
    ) -> Self {
        Self { grabber, cache, covers, unavailable: false }
    }

    /// The cache it saves into.
    pub fn cache(&self) -> &ThumbnailCache {
        &self.cache
    }

    /// Whether videos can be grabbed (as far as it knows).
    pub fn can_grab(&self) -> bool {
        !self.unavailable
    }

    /// The pictures of the video `identity` at `path`, `length` long. Between tries it asks
    /// `wanted` whether to go on.
    pub fn video(
        &mut self,
        identity: &str,
        path: &Path,
        length: Option<Duration>,
        wanted: &mut dyn FnMut() -> bool,
    ) -> Made {
        let mut best = Best::default();
        for at in pick::times(length) {
            if !wanted() {
                return Made::Stopped;
            }
            let grabbed = self.grabber.grab(path, at);
            if grabbed == Grab::Unavailable {
                self.unavailable = true;
                return Made::Stopped;
            }
            if let Grab::Frame(frame) = grabbed
                && best.offer(frame)
            {
                break;
            }
        }
        let picture = best.into_frame().and_then(|frame| thumbnail_cache::frame_picture(&frame));
        self.save(Source::Video(identity), picture)
    }

    /// The pictures of a song whose cover is cached as `artwork`.
    pub fn cover(&self, artwork: Option<&str>) -> Made {
        let Some((key, covers)) = artwork.zip(self.covers.as_ref()) else { return Made::Nothing };
        let picture = read_picture(&covers.path(key));
        self.save(Source::Cover(key), picture)
    }

    fn save(&self, source: Source<'_>, picture: Option<DynamicImage>) -> Made {
        let Some(picture) = picture else { return Made::Nothing };
        match self.cache.store(source, &picture) {
            Ok(images) => Made::Pictures(images),
            Err(error) => {
                tracing::warn!("library: couldn't save a thumbnail: {error}");
                Made::Stopped
            }
        }
    }
}

/// The picture in the file at `path`, unless it is missing, too large, or not a picture.
fn read_picture(path: &Path) -> Option<DynamicImage> {
    let size = std::fs::metadata(path).ok()?.len();
    if size > MAX_SOURCE_BYTES {
        return None;
    }
    image::open(path)
        .inspect_err(|error| {
            tracing::debug!("library: no cover in {}: {error}", crate::logging::path(path));
        })
        .ok()
}
