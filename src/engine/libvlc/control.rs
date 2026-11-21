//! Carries out the player's commands with libVLC calls, on the worker thread.

use std::ffi::{CString, c_int};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use super::worker::{Refresh, Worker};
use crate::engine::backend::{BackendEvent, Command};
use crate::engine::media_info::{TrackId, TrackKind};
use crate::engine::values::{BackendSeek, Offset, StepDirection};

/// The frame rate assumed for a backward step when libVLC doesn't know the real one.
const FALLBACK_FPS: f32 = 25.0;
/// libVLC's track id for "off".
const NO_TRACK: c_int = -1;
/// libVLC's volume for 100%.
const FULL_VOLUME: f32 = 100.0;
/// `libvlc_media_slave_type_subtitle`.
const SLAVE_SUBTITLE: c_int = 0;

impl Worker {
    /// Runs one command.
    pub(super) fn execute(&mut self, command: Command) {
        let vlc = Arc::clone(&self.vlc);
        let api = &vlc.api;
        let player = self.handles.player;
        // SAFETY (every call below): the player is live for the worker's whole life, and each
        // call matches its libVLC 3 signature.
        unsafe {
            match command {
                Command::Open { media, generation } => self.open(media, generation),
                Command::Play => _ = (api.libvlc_media_player_play)(player),
                Command::Pause => (api.libvlc_media_player_set_pause)(player, 1),
                Command::Stop => (api.libvlc_media_player_stop)(player),
                Command::Seek(to) => self.seek(to),
                Command::FrameStep(direction) => self.frame_step(direction),
                Command::SetRate(rate) => {
                    _ = (api.libvlc_media_player_set_rate)(player, rate.get());
                }
                Command::SetVolume(volume) => {
                    let level = (volume.get() * FULL_VOLUME).round() as c_int;
                    _ = (api.libvlc_audio_set_volume)(player, level);
                }
                Command::SetMute(muted) => (api.libvlc_audio_set_mute)(player, c_int::from(muted)),
                Command::SelectTrack(kind, track) => self.select_track(kind, track),
                Command::SetAudioDelay(delay) => {
                    self.audio_delay = delay;
                    self.apply_delays();
                }
                Command::SetSubtitleDelay(delay) => {
                    self.subtitle_delay = delay;
                    self.apply_delays();
                }
                Command::AddSubtitle(path) => self.add_subtitle(&path),
            }
        }
    }

    /// Sets both delays on the playing media. libVLC resets them for every new media, so this
    /// runs again once each open starts playing.
    pub(super) fn apply_delays(&self) {
        let api = &self.vlc.api;
        // SAFETY: the player is live.
        unsafe {
            (api.libvlc_audio_set_delay)(self.handles.player, micros(self.audio_delay));
            (api.libvlc_video_set_spu_delay)(self.handles.player, micros(self.subtitle_delay));
        }
    }

    fn seek(&self, to: BackendSeek) {
        self.shared.seeking.store(true, Ordering::Relaxed);
        let api = &self.vlc.api;
        // SAFETY: the player is live.
        unsafe {
            match to {
                BackendSeek::Time(time) => {
                    (api.libvlc_media_player_set_time)(self.handles.player, millis(time));
                }
                BackendSeek::Fraction(fraction) => {
                    (api.libvlc_media_player_set_position)(self.handles.player, fraction);
                }
            }
        }
    }

    fn frame_step(&self, direction: StepDirection) {
        let api = &self.vlc.api;
        let player = self.handles.player;
        // SAFETY: the player is live.
        unsafe {
            match direction {
                // `next_frame` pauses by itself.
                StepDirection::Forward => (api.libvlc_media_player_next_frame)(player),
                // libVLC has no backward step: pause and seek back one frame's time.
                StepDirection::Backward => {
                    (api.libvlc_media_player_set_pause)(player, 1);
                    let fps = Some((api.libvlc_media_player_get_fps)(player))
                        .filter(|fps| *fps > 0.0)
                        .unwrap_or(FALLBACK_FPS);
                    let now = (api.libvlc_media_player_get_time)(player).max(0);
                    let frame = Duration::from_secs_f32(1.0 / fps);
                    (api.libvlc_media_player_set_time)(player, (now - millis(frame)).max(0));
                }
            }
        }
        self.shared.send(BackendEvent::FrameStepped);
    }

    fn select_track(&mut self, kind: TrackKind, track: Option<TrackId>) {
        let api = &self.vlc.api;
        let id = track.map_or(NO_TRACK, |track| track.0);
        let select = match kind {
            TrackKind::Audio => api.libvlc_audio_set_track,
            TrackKind::Video => api.libvlc_video_set_track,
            TrackKind::Subtitle => api.libvlc_video_set_spu,
        };
        // SAFETY: the player is live.
        if unsafe { select(self.handles.player, id) } != 0 {
            tracing::warn!("libvlc: couldn't select {kind:?} track {id}");
        }
        self.refresh(Refresh::Tracks);
    }
}

impl Worker {
    /// Adds a subtitle file as a slave of the open media and selects it. libVLC takes a URI.
    fn add_subtitle(&mut self, path: &Path) {
        let uri =
            url::Url::from_file_path(path).ok().and_then(|uri| CString::new(uri.as_str()).ok());
        let Some(uri) = uri else {
            tracing::warn!("libvlc: can't make a subtitle URI from {}", crate::logging::path(path));
            return;
        };
        let api = &self.vlc.api;
        // SAFETY: the player is live and `uri` is NUL-terminated for the call.
        let added = unsafe {
            (api.libvlc_media_player_add_slave)(
                self.handles.player,
                SLAVE_SUBTITLE,
                uri.as_ptr(),
                true,
            )
        };
        if added != 0 {
            tracing::warn!("libvlc: couldn't add the subtitle: {}", self.vlc.last_error());
        }
        self.refresh(Refresh::Tracks);
    }
}

/// `time` in whole milliseconds, as libVLC takes it.
fn millis(time: Duration) -> i64 {
    i64::try_from(time.as_millis()).unwrap_or(i64::MAX)
}

/// `offset` in microseconds, as libVLC's delays take it.
fn micros(offset: Offset) -> i64 {
    offset.as_millis().saturating_mul(1000)
}
