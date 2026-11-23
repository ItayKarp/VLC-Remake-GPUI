//! Hardware or software decoding, and switching to software when subtitles show.
//!
//! libVLC 3 draws subtitles by blending them onto the decoded picture, and it can't blend onto
//! hardware frames (VideoToolbox's `CVPN`: "no matching alpha blending routine"), so subtitles
//! over hardware-decoded video never appear. Software decoding costs far more CPU, so media
//! opens with hardware decoding, and only once a subtitle shows over video does the worker
//! reopen it in software where it is ([`super::reopen`]).
//!
//! A subtitle must stay shown for [`SWITCH_DELAY`] first: libVLC picks one by itself as playback
//! starts, and the player's auto-selection may turn it off right after. It stays in software
//! when subtitles are turned off again, so toggling them doesn't reopen the media each time.

use std::ffi::{CString, c_int};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use super::api::Media;
use super::reopen::{Output, Restore};
use super::worker::Worker;
use crate::engine::tracks::{TrackKind, Tracks};
use crate::engine::values::Start;

/// Media options that decode in software. `any` keeps the other decoders (subtitles) usable.
pub(super) const SOFTWARE_DECODING: [&str; 2] = [":codec=avcodec,any", ":avcodec-hw=none"];
/// How long a subtitle stays shown before the media is reopened to draw it.
const SWITCH_DELAY: Duration = Duration::from_millis(300);
/// `libvlc_media_slave_type_subtitle`.
const SLAVE_SUBTITLE: c_int = 0;
/// `libvlc_media_slave_type_audio`.
const SLAVE_AUDIO: c_int = 1;

/// A file added to the open media: a subtitle the user added or one found beside the video
/// (S6.1), or an audio track (S4.8.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slave {
    /// Whether it is subtitles or audio.
    pub kind: SlaveKind,
    /// The file.
    pub path: std::path::PathBuf,
}

/// What an added file holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlaveKind {
    /// Subtitles.
    Subtitle,
    /// An audio track.
    Audio,
}

impl SlaveKind {
    /// libVLC's `libvlc_media_slave_type_t` for it.
    pub(super) fn libvlc_type(self) -> c_int {
        match self {
            Self::Subtitle => SLAVE_SUBTITLE,
            Self::Audio => SLAVE_AUDIO,
        }
    }

    /// The kind of track the file adds.
    pub(super) fn track_kind(self) -> TrackKind {
        match self {
            Self::Subtitle => TrackKind::Subtitle,
            Self::Audio => TrackKind::Audio,
        }
    }
}

/// How the open media's video is decoded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Decoding {
    /// By the GPU, where libVLC can (VideoToolbox, DXVA, VA-API).
    #[default]
    Hardware,
    /// By the CPU.
    Software,
}

impl Worker {
    /// Keeps the crash marker ([`super::hardware`]) while the GPU decodes this media's video.
    pub(super) fn mark_hardware(&self, tracks: &Tracks) {
        let gpu_decodes = self.decoding == Decoding::Hardware && tracks.video.is_some();
        super::hardware::decoding(self.source.as_ref().filter(|_| gpu_decodes));
    }

    /// Notes whether `tracks` need software decoding, so the switch happens once that has
    /// lasted [`SWITCH_DELAY`].
    pub(super) fn watch_subtitles(&mut self, tracks: &Tracks, now: Instant) {
        self.switch_due =
            self.needs_software(tracks).then(|| self.switch_due.unwrap_or(now + SWITCH_DELAY));
    }

    /// How long until the switch is due, if one is.
    pub(super) fn until_switch(&self, now: Instant) -> Option<Duration> {
        self.switch_due.map(|due| due.saturating_duration_since(now))
    }

    /// Switches to software decoding if it is due and a subtitle still shows.
    pub(super) fn switch_if_due(&mut self, now: Instant) {
        if self.switch_due.is_none_or(|due| due > now) {
            return;
        }
        self.switch_due = None;
        let tracks = Tracks {
            all: Vec::new(),
            audio: self.selected(TrackKind::Audio),
            video: self.selected(TrackKind::Video),
            subtitle: self.selected(TrackKind::Subtitle),
        };
        if self.needs_software(&tracks) {
            self.switch_to_software(&tracks);
        }
    }

    /// Whether `tracks` show a subtitle over hardware-decoded video that has started playing.
    fn needs_software(&self, tracks: &Tracks) -> bool {
        self.decoding == Decoding::Hardware
            && tracks.subtitle.is_some()
            && tracks.video.is_some()
            && self.shared.started.load(Ordering::Relaxed)
            && !self.shared.switching.load(Ordering::Relaxed)
    }

    /// Reopens the media in software where it is, with `tracks`' selection.
    fn switch_to_software(&mut self, tracks: &Tracks) {
        tracing::info!("libvlc: a subtitle shows; decoding in software so it can be drawn");
        self.decoding = Decoding::Software;
        super::hardware::decoding(None);
        self.reopen(Restore { audio: tracks.audio, subtitle: tracks.subtitle }, Output::Kept);
    }

    /// Adds media options, as `prepare_video` and the switch do.
    pub(super) fn add_option(&self, media: *mut Media, options: &[&str]) {
        for option in options {
            let Ok(option) = CString::new(*option) else { continue };
            // SAFETY: the media is live and the option is NUL-terminated.
            unsafe { (self.vlc.api.libvlc_media_add_option)(media, option.as_ptr()) };
        }
    }

    /// Starts new media where and how `start` says: libVLC's own `start-time` and
    /// `start-paused`, so a restored session (S3.4) neither plays a moment of sound nor shows
    /// the first frames before the saved position.
    pub(super) fn add_start_options(&self, media: *mut Media, start: Start) {
        if !start.at.is_zero() {
            self.add_option(media, &[&format!(":start-time={:.3}", start.at.as_secs_f64())]);
        }
        if start.paused {
            self.add_option(media, &[":start-paused"]);
        }
    }
}
