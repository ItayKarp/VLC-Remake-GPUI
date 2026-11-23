//! Carries out the player's commands with libVLC calls, on the worker thread.

use std::ffi::{CStr, c_int};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use super::decoding::{Slave, SlaveKind};
use super::reopen::file_uri;
use super::worker::{Refresh, Worker};
use crate::engine::backend::{BackendEvent, Command};
use crate::engine::state::PlaybackState;
use crate::engine::tracks::{TrackId, TrackKind};
use crate::engine::values::{BackendSeek, Offset, StepDirection};

/// The frame rate assumed for a backward step when libVLC doesn't know the real one.
const FALLBACK_FPS: f32 = 25.0;
/// libVLC's track id for "off".
const NO_TRACK: c_int = -1;
/// libVLC's volume for 100%.
const FULL_VOLUME: f32 = 100.0;

impl Worker {
    /// Stops playback; the GPU no longer decodes anything.
    fn stop(&mut self) {
        // A stop during a switch to software decoding must reach the player.
        self.shared.switching.store(false, Ordering::Relaxed);
        self.end_hold();
        // SAFETY: the player is live for the worker's whole life.
        unsafe { (self.vlc.api.libvlc_media_player_stop)(self.handles.player) };
        super::hardware::decoding(None);
    }

    /// Plays; media held for its first picture after a reopen just keeps playing, unmuted.
    fn play(&mut self) {
        if self.end_hold() {
            self.shared.paused.store(false, Ordering::Relaxed);
            self.shared.send(BackendEvent::State(PlaybackState::Playing));
        } else {
            // SAFETY: the player is live.
            _ = unsafe { (self.vlc.api.libvlc_media_player_play)(self.handles.player) };
        }
    }

    /// Pauses, ending a hold for the first picture where it is.
    fn pause(&mut self) {
        self.pause_held();
        // SAFETY: the player is live.
        unsafe { (self.vlc.api.libvlc_media_player_set_pause)(self.handles.player, 1) };
    }

    /// Mutes or unmutes, or keeps it for after a hold for the first picture, which is muted.
    fn set_mute(&mut self, muted: bool) {
        if !self.hold_mute(muted) {
            // SAFETY: the player is live.
            unsafe {
                (self.vlc.api.libvlc_audio_set_mute)(self.handles.player, c_int::from(muted));
            };
        }
    }

    /// Runs one command.
    pub(super) fn execute(&mut self, command: Command) {
        let vlc = Arc::clone(&self.vlc);
        let api = &vlc.api;
        let player = self.handles.player;
        // SAFETY (every call below): the player is live for the worker's whole life, and each
        // call matches its libVLC 3 signature.
        unsafe {
            match command {
                Command::Open { media, start, generation } => self.open(media, start, generation),
                Command::Play => self.play(),
                Command::Pause => self.pause(),
                Command::Stop => self.stop(),
                Command::Seek(to) => self.seek(to),
                Command::FrameStep(direction) => self.frame_step(direction),
                Command::SetRate(rate) => {
                    _ = (api.libvlc_media_player_set_rate)(player, rate.get());
                }
                Command::SetVolume(volume) => {
                    let level = (volume.get() * FULL_VOLUME).round() as c_int;
                    _ = (api.libvlc_audio_set_volume)(player, level);
                }
                Command::SetMute(muted) => self.set_mute(muted),
                Command::SelectTrack(kind, track) => self.select_track(kind, track),
                Command::SetAudioDelay(delay) => {
                    self.audio_delay = delay;
                    self.apply_delays();
                }
                Command::SetSubtitleDelay(delay) => {
                    self.subtitle_delay = delay;
                    self.apply_delays();
                }
                Command::AddSubtitle(path) => self.add_slave(SUBTITLE, path, Attach::Shown),
                Command::LoadSubtitle(path) => self.add_slave(SUBTITLE, path, Attach::Listed),
                Command::AddAudio(path) => self.add_slave(SlaveKind::Audio, path, Attach::Shown),
                Command::SetAudioDevice(device) => self.set_audio_device(&device),
                Command::RefreshAudioDevices => self.poll_devices(),
                Command::SetSpeakerSetup(setup) => self.set_speaker_setup(setup),
                Command::SetMediaOptions(options) => self.media_options = options,
                Command::SetEqualizer(equalizer) => self.set_equalizer(equalizer.as_ref()),
                Command::SetPicture(picture) => self.set_picture(picture),
                Command::SetVideoFilters(filters) => self.set_video_filters(filters),
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
        self.defaults.commanded(kind);
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

/// Shortens the subtitle commands' lines.
const SUBTITLE: SlaveKind = SlaveKind::Subtitle;

/// Whether an added file's track is selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Attach {
    /// Selected: a file the user added.
    Shown,
    /// Only listed: a file found beside the video, which the player's auto-selection may pick.
    Listed,
}

impl Worker {
    /// Adds a subtitle or audio file as a slave of the open media, selected as `attach` says.
    /// libVLC takes a URI.
    fn add_slave(&mut self, kind: SlaveKind, path: PathBuf, attach: Attach) {
        let Some(uri) = file_uri(&path) else {
            tracing::warn!("libvlc: can't make a URI from {}", crate::logging::path(&path));
            return;
        };
        if attach == Attach::Shown {
            self.defaults.commanded(kind.track_kind());
        }
        if self.attach_slave(kind, &uri, attach) {
            self.slaves.push(Slave { kind, path });
        }
        self.refresh(Refresh::Tracks);
    }

    /// Hands `uri` to libVLC as a `kind` slave. False (logged) when libVLC refuses it.
    fn attach_slave(&self, kind: SlaveKind, uri: &CStr, attach: Attach) -> bool {
        let api = &self.vlc.api;
        // SAFETY: the player is live and `uri` is NUL-terminated for the call.
        let added = unsafe {
            (api.libvlc_media_player_add_slave)(
                self.handles.player,
                kind.libvlc_type(),
                uri.as_ptr(),
                attach == Attach::Shown,
            )
        };
        if added != 0 {
            tracing::warn!("libvlc: couldn't add the {kind:?} file: {}", self.vlc.last_error());
        }
        added == 0
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
