//! What is on a disc, read without playing it (S8.9.1): an audio CD's tracks.

use std::time::Duration;

pub use super::libvlc::disc_tracks::audio_cd_tracks;

/// One track of an audio CD.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CdTrack {
    /// Its title, from the disc's CD-Text, or libVLC's "Audio CD - Track 01" without it.
    pub title: Option<String>,
    /// Its artist, from the disc's CD-Text.
    pub artist: Option<String>,
    /// How long it is.
    pub duration: Option<Duration>,
}
