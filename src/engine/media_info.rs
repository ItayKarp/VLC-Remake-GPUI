//! What the backend learns about the open media besides its tracks ([`super::tracks`]): its
//! chapters, metadata and video size.

use std::time::Duration;

/// The displayed size of the video, after pixel aspect ratio and rotation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VideoSize {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// A chapter of the media.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chapter {
    /// Its title, if the container names it.
    pub title: Option<String>,
    /// Where it starts.
    pub start: Duration,
}

/// Descriptive tags read from the media.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Metadata {
    /// The title tag.
    pub title: Option<String>,
    /// The artist tag.
    pub artist: Option<String>,
    /// The album tag.
    pub album: Option<String>,
}
