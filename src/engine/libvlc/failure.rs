//! Names a libVLC failure. libVLC 3 reports only "an error happened", so the kind is inferred
//! from the source and from how far playback got.

use std::sync::atomic::Ordering;

use super::worker::Worker;
use crate::app::media::Media;
use crate::engine::backend::BackendEvent;
use crate::engine::preflight;
use crate::engine::state::{ErrorKind, PlaybackState};

/// Why playback of `source` failed, given whether it had `started` playing.
///
/// A local file that is gone or unreadable mid-play was lost (deleted, renamed, drive
/// unplugged); one still intact was corrupt. A stream that played and failed lost its source.
/// Anything that never started couldn't be opened.
pub fn classify(source: Option<&Media>, started: bool) -> ErrorKind {
    let check = source.and_then(Media::as_path).map(preflight::check);
    match (check, started) {
        (Some(Err(kind)), false) => kind,
        (Some(Err(_)) | None, true) => ErrorKind::SourceLost,
        (Some(Ok(())), true) => ErrorKind::Corrupt,
        (_, false) => ErrorKind::OpenFailed,
    }
}

impl Worker {
    /// libVLC reached the end: the media ended, or it had nothing it could play.
    pub(super) fn ended(&mut self) {
        super::hardware::decoding(None);
        self.recording_ends();
        let state = if self.track_count() == 0 {
            PlaybackState::Error(ErrorKind::NoPlayableStreams)
        } else {
            PlaybackState::Ended
        };
        self.shared.send(BackendEvent::State(state));
    }

    /// libVLC reported an error: says which, as far as it can be told.
    pub(super) fn failed(&mut self) {
        super::hardware::decoding(None);
        self.recording_ends();
        let started = self.shared.started.load(Ordering::Relaxed);
        let kind = classify(self.source.as_ref(), started);
        tracing::warn!("libvlc: playback failed ({kind:?}): {}", self.vlc.last_error());
        self.shared.send(BackendEvent::State(PlaybackState::Error(kind)));
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn a_vanished_file_was_lost_and_an_intact_one_corrupt() {
        let dir = std::env::temp_dir().join(format!("rvlc-failure-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let intact = dir.join("intact.mkv");
        fs::write(&intact, b"data").unwrap();
        let gone = Media::File(dir.join("gone.mkv"));
        assert_eq!(classify(Some(&gone), true), ErrorKind::SourceLost);
        assert_eq!(classify(Some(&gone), false), ErrorKind::NotFound);
        assert_eq!(classify(Some(&Media::File(intact.clone())), true), ErrorKind::Corrupt);
        assert_eq!(classify(Some(&Media::File(intact)), false), ErrorKind::OpenFailed);
    }

    #[test]
    fn streams_fail_to_open_or_lose_their_source() {
        let stream = Media::Url("https://example.com/live.m3u8".into());
        assert_eq!(classify(Some(&stream), false), ErrorKind::OpenFailed);
        assert_eq!(classify(Some(&stream), true), ErrorKind::SourceLost);
    }
}
