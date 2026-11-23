//! The commands the UI gives the [`Player`].

use std::path::Path;
use std::time::Instant;

use gpui::Context;

use super::{Info, Intent, Player, PlayerEvent};
use crate::app::media::Media;
use crate::engine::preflight;
use crate::engine::state::PlaybackState;
use crate::engine::tracks::TrackKind;
use crate::engine::values::{BackendSeek, Rate, SeekTarget, Start, StepDirection};

impl Player {
    /// Opens `media` and starts playing it, cancelling whatever was open. A local path that
    /// can't be media (missing, empty, a folder or device) fails at once with an error state.
    pub fn open(&mut self, media: Media, cx: &mut Context<Self>) {
        self.open_at(media, Start::default(), cx);
    }

    /// [`Self::open`], starting where `start` says: from a position, and paused there rather
    /// than playing (restoring the last session, S3.4).
    pub fn open_at(&mut self, media: Media, start: Start, cx: &mut Context<Self>) {
        self.generation = self.generation.next();
        self.intent = if start.paused { Intent::Pause } else { Intent::Play };
        self.audio.user_took_over();
        self.info = Info::default();
        self.push_subtitle_delay();
        self.push_audio_delay();
        self.track_choices.forget_media();
        self.position.reset();
        self.seeks.reset();
        self.steps.reset();
        let preflight = media.as_path().map(preflight::check);
        self.media = Some(media);
        // Not `transition`: that ignores repeats, and reopening while opening must still tell
        // the UI to forget the previous media.
        self.state = PlaybackState::Opening;
        cx.emit(PlayerEvent::StateChanged(PlaybackState::Opening));
        if let Some(Err(kind)) = preflight {
            // Whatever played before must not keep playing behind the error; its late events
            // carry the old generation and are dropped.
            self.with_backend(|backend| backend.stop());
            self.transition(PlaybackState::Error(kind), cx);
            return;
        }
        if let (Some(backend), Some(media)) = (self.backend.as_deref_mut(), &self.media) {
            backend.open(media, start, self.generation);
        }
        self.find_sidecars(cx);
    }

    /// Plays: resumes when paused, restarts the media when it ended or was stopped.
    pub fn play(&mut self, cx: &mut Context<Self>) {
        self.intent = Intent::Play;
        self.audio.user_took_over();
        match self.state {
            PlaybackState::Paused => self.with_backend(|backend| backend.play()),
            PlaybackState::Ended | PlaybackState::Stopped => {
                if let Some(media) = self.media.clone() {
                    self.open(media, cx);
                }
            }
            // Opening and buffering play on their own; the intent covers a pause asked for
            // earlier.
            _ => {}
        }
    }

    /// Pauses, or pauses once opening or buffering ends. Ignored when the media can't pause.
    pub fn pause(&mut self, _cx: &mut Context<Self>) {
        if !self.state.is_active() || self.info.pausable == Some(false) {
            return;
        }
        self.intent = Intent::Pause;
        self.audio.user_took_over();
        if self.state == PlaybackState::Playing {
            self.with_backend(|backend| backend.pause());
        }
    }

    /// Pauses when playing (or about to), plays otherwise.
    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        if self.state.is_active() && self.intent == Intent::Play {
            self.pause(cx);
        } else {
            self.play(cx);
        }
    }

    /// Stops playback. The media stays selected, so [`Self::play`] starts it again.
    pub fn stop(&mut self, _cx: &mut Context<Self>) {
        if !self.state.is_active() && self.state != PlaybackState::Ended {
            return;
        }
        self.seeks.cancel();
        self.steps.reset();
        self.audio.user_took_over();
        self.with_backend(|backend| backend.stop());
    }

    /// Seeks. While another seek is in flight only the latest target is kept, and a seek before
    /// the media says it is seekable waits for that (or is dropped if it never becomes so).
    /// libVLC can't seek while recording, so that is refused with [`PlayerEvent::SeekRefused`].
    pub fn seek(&mut self, target: SeekTarget, cx: &mut Context<Self>) {
        if !self.state.is_active() || self.seeks.is_seekable() == Some(false) {
            return;
        }
        if self.recording.is_some() {
            cx.emit(PlayerEvent::SeekRefused);
            return;
        }
        let Some(seek) = self.resolve(target) else { return };
        if let BackendSeek::Time(time) = seek {
            let shown = self.position.seeked(time, Instant::now());
            cx.emit(PlayerEvent::PositionChanged(shown));
        }
        if let Some(seek) = self.seeks.request(seek) {
            self.with_backend(|backend| backend.seek(seek));
        }
    }

    /// Pauses and shows the next or previous frame. Steps asked for while one is running queue
    /// up, to a limit.
    pub fn frame_step(&mut self, direction: StepDirection, _cx: &mut Context<Self>) {
        if !self.state.is_active() {
            return;
        }
        self.intent = Intent::Pause;
        if let Some(direction) = self.steps.request(direction) {
            self.with_backend(|backend| backend.frame_step(direction));
        }
    }

    /// Sets the playback speed.
    pub fn set_rate(&mut self, rate: Rate, cx: &mut Context<Self>) {
        self.controls.rate = rate;
        self.with_backend(|backend| backend.set_rate(rate));
        cx.notify();
    }

    /// Adds the audio file at `path` to the open media as a track and plays it (S4.8.1's Add
    /// Audio Tracks). Returns `false`, and does nothing, when no media is open to add it to.
    pub fn add_audio(&mut self, path: &Path, cx: &mut Context<Self>) -> bool {
        if !self.state.is_active() {
            return false;
        }
        // The backend plays the added file: auto-selection must not switch away from it.
        self.track_choices.user_picked(TrackKind::Audio);
        self.with_backend(|backend| backend.add_audio(path));
        cx.emit(PlayerEvent::TracksChanged);
        cx.notify();
        true
    }

    /// The seek the backend should do for `target`, given what is known about the media.
    fn resolve(&self, target: SeekTarget) -> Option<BackendSeek> {
        let duration = self.position.duration();
        let clamp = |time| duration.map_or(time, |duration| std::cmp::min(time, duration));
        match target {
            SeekTarget::Absolute(time) => Some(BackendSeek::Time(clamp(time))),
            SeekTarget::Relative(offset) => {
                Some(BackendSeek::Time(clamp(offset.apply(self.position.shown()))))
            }
            SeekTarget::Fraction(fraction) if fraction.is_nan() => None,
            SeekTarget::Fraction(fraction) => {
                let fraction = fraction.clamp(0.0, 1.0);
                Some(duration.map_or(BackendSeek::Fraction(fraction), |duration| {
                    BackendSeek::Time(duration.mul_f32(fraction))
                }))
            }
        }
    }
}
