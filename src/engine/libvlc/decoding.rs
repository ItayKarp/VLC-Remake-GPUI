//! Hardware or software decoding, and switching to software when subtitles show.
//!
//! libVLC 3 draws subtitles by blending them onto the decoded picture, and it can't blend onto
//! hardware frames (VideoToolbox's `CVPN`: "no matching alpha blending routine"), so subtitles
//! over hardware-decoded video never appear. Software decoding costs far more CPU, so media
//! opens with hardware decoding, and only once a subtitle shows over video does the worker
//! reopen it in software: at the same position, paused if it was, with the same tracks and the
//! subtitle files the user added. The player doesn't see the reopen: the callback hides its
//! opening and stopping, and the track list waits until the selection is restored.
//!
//! A subtitle must stay shown for [`SWITCH_DELAY`] first: libVLC picks one by itself as playback
//! starts, and the player's auto-selection may turn it off right after. It stays in software
//! when subtitles are turned off again, so toggling them doesn't reopen the media each time.

use std::ffi::{CString, c_int};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use super::api::Media;
use super::worker::Worker;
use crate::engine::tracks::{TrackId, TrackKind, Tracks};
use crate::engine::values::Start;

/// Media options that decode in software. `any` keeps the other decoders (subtitles) usable.
pub(super) const SOFTWARE_DECODING: [&str; 2] = [":codec=avcodec,any", ":avcodec-hw=none"];
/// How long a subtitle stays shown before the media is reopened to draw it.
const SWITCH_DELAY: Duration = Duration::from_millis(300);
/// `libvlc_media_slave_type_subtitle`.
pub(super) const SLAVE_SUBTITLE: c_int = 0;
/// `libvlc_media_slave_priority`'s highest value, as for files the user picks.
const SLAVE_PRIORITY_USER: u32 = 4;

/// How the open media's video is decoded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Decoding {
    /// By the GPU, where libVLC can (VideoToolbox, DXVA, VA-API).
    #[default]
    Hardware,
    /// By the CPU.
    Software,
}

/// The tracks to select again once the reopened media plays.
#[derive(Clone, Copy, Debug)]
pub struct Restore {
    audio: Option<TrackId>,
    subtitle: Option<TrackId>,
}

impl Worker {
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

    /// Reopens the media in software where it is, to restore `tracks`' selection once it
    /// plays.
    fn switch_to_software(&mut self, tracks: &Tracks) {
        let Some(source) = self.source.clone() else { return };
        let media = self.new_media(&source);
        if media.is_null() {
            return;
        }
        tracing::info!("libvlc: a subtitle shows; decoding in software so it can be drawn");
        // Not parsed again: the sink keeps the display size from the open, and parsing the
        // playing file before `set_media` stalls libVLC's input thread when it stops.
        self.add_option(media, &SOFTWARE_DECODING);
        self.add_option(media, &[&self.resume_option()]);
        if self.shared.paused.load(Ordering::Relaxed) {
            self.add_option(media, &[":start-paused"]);
        }
        self.add_slaves(media);
        self.decoding = Decoding::Software;
        self.restore = Some(Restore { audio: tracks.audio, subtitle: tracks.subtitle });
        self.shared.switching.store(true, Ordering::Relaxed);
        self.replace_media(media);
        self.play_media();
    }

    /// Selects the tracks that played before the switch, once the reopened media plays.
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

    /// Starts the reopened media where the old one was.
    fn resume_option(&self) -> String {
        // SAFETY: the player is live.
        let millis = unsafe { (self.vlc.api.libvlc_media_player_get_time)(self.handles.player) };
        format!(":start-time={:.3}", millis.max(0) as f64 / 1000.0)
    }

    /// Opens the subtitle files the user added with the reopened media. They get the same
    /// track ids as before, since they are added in the same order after the same tracks.
    fn add_slaves(&self, media: *mut Media) {
        for uri in self.slaves.iter().filter_map(|path| subtitle_uri(path)) {
            // SAFETY: the media is live and `uri` is NUL-terminated.
            let added = unsafe {
                (self.vlc.api.libvlc_media_slaves_add)(
                    media,
                    SLAVE_SUBTITLE,
                    SLAVE_PRIORITY_USER,
                    uri.as_ptr(),
                )
            };
            if added != 0 {
                tracing::warn!("libvlc: couldn't carry a subtitle file over");
            }
        }
    }
}

/// The `file://` URI libVLC takes for a subtitle file at `path`.
pub(super) fn subtitle_uri(path: &Path) -> Option<CString> {
    url::Url::from_file_path(path).ok().and_then(|uri| CString::new(uri.as_str()).ok())
}
