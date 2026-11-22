//! One picture of a video at a chosen time (S4.4.1), for the library's thumbnails. A
//! [`FrameGrabber`] blocks while it decodes, so it runs on the thumbnailer's thread;
//! [`default_grabber`] grabs with libVLC, loaded the first time it is used.

use std::path::Path;
use std::time::Duration;

pub use super::preview::PreviewImage as Frame;

/// The largest picture a grab asks for: the grid tile at 2× scale (`library::thumbnail_cache`).
pub const MAX_WIDTH: u32 = 640;
/// The largest grab height.
pub const MAX_HEIGHT: u32 = 360;

/// How one grab went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Grab {
    /// The picture at the asked time, fitting [`MAX_WIDTH`] × [`MAX_HEIGHT`].
    Frame(Frame),
    /// No picture: the file has no video, ends before that time, is damaged, or took too long.
    Nothing,
    /// There is no backend to grab with: nothing should be marked as tried.
    Unavailable,
}

/// Grabs pictures from video files. Every call blocks until the picture arrives or it gives up.
pub trait FrameGrabber: Send {
    /// The picture of the file at `path`, `at` from its start.
    fn grab(&mut self, path: &Path, at: Duration) -> Grab;
}

/// The grabber to use: libVLC's, which loads libVLC the first time it grabs.
pub fn default_grabber() -> Box<dyn FrameGrabber> {
    Box::new(super::libvlc::thumbnail::LibVlcThumbnails::default())
}

/// A grabber without a backend: every grab is [`Grab::Unavailable`].
pub struct NoFrames;

impl FrameGrabber for NoFrames {
    fn grab(&mut self, _: &Path, _: Duration) -> Grab {
        Grab::Unavailable
    }
}
