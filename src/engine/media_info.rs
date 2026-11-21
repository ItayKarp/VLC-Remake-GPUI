//! What the backend learns about the open media: its tracks, chapters, metadata and video size.

use std::time::Duration;

/// The kind of elementary stream a track carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TrackKind {
    /// Sound.
    Audio,
    /// Pictures.
    Video,
    /// Subtitles or captions.
    Subtitle,
}

/// The backend's id for a track, unique within one media.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TrackId(pub i32);

/// One track of the media.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Track {
    /// The backend's id, passed back to select it.
    pub id: TrackId,
    /// What it carries.
    pub kind: TrackKind,
    /// The name the container gives it, if any.
    pub name: Option<String>,
    /// Its language code (`en`, `fra`), if the container says.
    pub language: Option<String>,
}

/// All tracks of the media and the ones playing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tracks {
    /// Every track, in the container's order.
    pub all: Vec<Track>,
    /// The playing audio track, or `None` when audio is off.
    pub audio: Option<TrackId>,
    /// The playing video track, or `None` when video is off.
    pub video: Option<TrackId>,
    /// The shown subtitle track, or `None` when subtitles are off.
    pub subtitle: Option<TrackId>,
}

impl Tracks {
    /// The selected track of `kind`.
    pub fn selected(&self, kind: TrackKind) -> Option<TrackId> {
        match kind {
            TrackKind::Audio => self.audio,
            TrackKind::Video => self.video,
            TrackKind::Subtitle => self.subtitle,
        }
    }

    /// Records `track` as the selection for `kind`.
    pub fn select(&mut self, kind: TrackKind, track: Option<TrackId>) {
        match kind {
            TrackKind::Audio => self.audio = track,
            TrackKind::Video => self.video = track,
            TrackKind::Subtitle => self.subtitle = track,
        }
    }
}

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
