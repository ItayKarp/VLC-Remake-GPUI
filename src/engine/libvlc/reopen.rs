//! Reopening the media playing where it is, unseen: for software decoding once a subtitle shows
//! ([`super::decoding`]), and for new video filters, which libVLC builds into a video output only
//! when it opens (`video_filters.rs`).
//!
//! The reopened media starts at the same position, paused if it was, with the same tracks and
//! the subtitle and audio files added to it. The player doesn't see it: the callback hides its
//! opening and stopping, and the track list waits until the selection is restored.
//!
//! libVLC keeps the video output for the next media when the format allows, with the filters
//! it was built with, so a reopen for new filters stops the player first, which lets it go.

use std::ffi::CString;
use std::path::Path;
use std::sync::atomic::Ordering;

use super::api::Media;
use super::decoding::{Decoding, SOFTWARE_DECODING};
use super::worker::Worker;
use crate::engine::tracks::{TrackId, TrackKind};

/// `libvlc_media_slave_priority`'s highest value, as for files the user picks.
const SLAVE_PRIORITY_USER: u32 = 4;

/// What happens to the video output on a reopen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output {
    /// Kept if libVLC can: decoding changes, the filters don't.
    Kept,
    /// Made anew, with the player's filters as they are now.
    Rebuilt,
}

/// The tracks to select again once the reopened media plays.
#[derive(Clone, Copy, Debug)]
pub struct Restore {
    /// The audio track.
    pub audio: Option<TrackId>,
    /// The subtitle track.
    pub subtitle: Option<TrackId>,
}

impl Worker {
    /// Reopens the media where it is, as [`Self::decoding`] says, with its video `output`, to
    /// select `tracks` again once it plays.
    pub(super) fn reopen(&mut self, tracks: Restore, output: Output) {
        let Some(source) = self.source.clone() else { return };
        let media = self.new_media(&source);
        if media.is_null() {
            return;
        }
        // Not parsed again: the sink keeps the display size from the open, and parsing the
        // playing file before `set_media` stalls libVLC's input thread when it stops.
        if self.decoding == Decoding::Software {
            self.add_option(media, &SOFTWARE_DECODING);
        }
        self.add_option(media, &[&self.resume_option()]);
        if self.shared.paused.load(Ordering::Relaxed) {
            self.add_option(media, &[":start-paused"]);
        }
        self.add_slaves(media);
        self.restore = Some(tracks);
        self.shared.switching.store(true, Ordering::Relaxed);
        if output == Output::Rebuilt {
            // SAFETY: the player is live; the stop it reports is hidden while switching.
            unsafe { (self.vlc.api.libvlc_media_player_stop)(self.handles.player) };
        }
        self.replace_media(media);
        self.play_media();
    }

    /// The tracks playing now, to select again after a reopen.
    pub(super) fn playing_tracks(&self) -> Restore {
        Restore {
            audio: self.selected(TrackKind::Audio),
            subtitle: self.selected(TrackKind::Subtitle),
        }
    }

    /// Selects the tracks that played before the reopen, once the reopened media plays.
    pub(super) fn restore_tracks(&mut self) {
        let Some(restore) = self.restore.take() else { return };
        let api = &self.vlc.api;
        // SAFETY: the player is live.
        unsafe {
            if let Some(audio) = restore.audio {
                (api.libvlc_audio_set_track)(self.handles.player, audio.0);
            }
            if let Some(subtitle) = restore.subtitle {
                (api.libvlc_video_set_spu)(self.handles.player, subtitle.0);
            }
        }
    }

    /// Starts the reopened media where the old one was.
    fn resume_option(&self) -> String {
        // SAFETY: the player is live.
        let millis = unsafe { (self.vlc.api.libvlc_media_player_get_time)(self.handles.player) };
        format!(":start-time={:.3}", millis.max(0) as f64 / 1000.0)
    }

    /// Opens the subtitle and audio files added to the old one with the reopened media. They get the
    /// same track ids as before, since they are added in the same order after the same tracks.
    fn add_slaves(&self, media: *mut Media) {
        for slave in &self.slaves {
            let Some(uri) = file_uri(&slave.path) else { continue };
            // SAFETY: the media is live and `uri` is NUL-terminated.
            let added = unsafe {
                (self.vlc.api.libvlc_media_slaves_add)(
                    media,
                    slave.kind.libvlc_type(),
                    SLAVE_PRIORITY_USER,
                    uri.as_ptr(),
                )
            };
            if added != 0 {
                tracing::warn!("libvlc: couldn't carry an added {:?} file over", slave.kind);
            }
        }
    }
}

/// The `file://` URI libVLC takes for a subtitle or audio file at `path`.
pub(super) fn file_uri(path: &Path) -> Option<CString> {
    url::Url::from_file_path(path).ok().and_then(|uri| CString::new(uri.as_str()).ok())
}
