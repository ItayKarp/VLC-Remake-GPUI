//! The background thumbnailer (S4.4): a thread of its own that makes the library's thumbnails and
//! keeps the cache, through the history thread's database connection.
//!
//! - [`queue`]: what is on screen first, then the backlog of videos without one (S4.4.4).
//! - [`pick`]: where in a video to look, and which frames are black or flat (S4.4.1).
//! - [`make`]: grabbing a frame, or cutting a song's cover, into every size.
//! - [`worker`]: the thread: answers what the views ask for, fills the cache in the background
//!   (paused while something plays), and evicts the least recently shown (S4.4.2).
//!
//! The grabber runs in this process with a timeout per grab. A file that crashes libVLC would
//! take the app with it, so a media is marked as given up on *before* each try: it isn't tried
//! again on the next launch.

pub mod make;
pub mod pick;
pub mod queue;
#[cfg(test)]
mod tests;
mod worker;

use std::path::{Path, PathBuf};

use crate::db::media::MediaId;
use crate::db::thumbnails::{Cached, SizeClass, Variant};
use crate::library::thumbnail_cache::ThumbnailCache;

pub use worker::spawn;

/// What the thumbnailer is asked to do.
#[derive(Debug)]
pub enum Command {
    /// These media are on screen, in order; what was on screen before and isn't now is dropped.
    Show(Vec<MediaId>),
    /// Something started or stopped playing: the background backlog waits while it plays.
    Playing(bool),
}

/// The thumbnailer's answer about one media the views asked for, or that it made in the
/// background.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answer {
    /// The media.
    pub media: MediaId,
    /// Its pictures, or `None` when it has none (the views show a placeholder).
    pub thumbnail: Option<Thumbnail>,
}

/// Where a media's pictures are, per size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Thumbnail {
    paths: Vec<(Variant, PathBuf)>,
}

impl Thumbnail {
    /// The pictures `images`, in `cache`.
    pub fn new(cache: &ThumbnailCache, images: &[Cached]) -> Self {
        let mut paths: Vec<_> =
            images.iter().map(|image| (image.variant, cache.path(&image.cache_key))).collect();
        // The same pictures compare equal however the database listed them.
        paths.sort_by_key(|(variant, _)| (variant.class.as_str(), variant.scale));
        Self { paths }
    }

    /// The picture for `class` at `scale` (1, or 2 for HiDPI), or the nearest scale there is.
    pub fn path(&self, class: SizeClass, scale: u8) -> Option<&Path> {
        let exact = self.paths.iter().find(|(v, _)| *v == Variant { class, scale });
        exact
            .or_else(|| self.paths.iter().find(|(variant, _)| variant.class == class))
            .map(|(_, path)| path.as_path())
    }
}

/// Where the thumbnailer's answers go.
pub type Report = Box<dyn Fn(Answer) + Send>;
