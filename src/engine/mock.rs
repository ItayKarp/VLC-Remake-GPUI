//! A backend that plays nothing (S1.2.7), so the player and the UI can be tested without media
//! files or libVLC.
//!
//! It records every command it receives and, unless built with [`MockMedia::manual`], answers
//! them the way a well-behaved backend would: opening reaches `Playing`, seeks complete at once
//! and seeking to the end ends playback. A [`MockHandle`] lets a test read the commands and
//! inject any event, including late events from an old open.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use super::audio::equalizer::Equalizer;
use super::audio::{OutputDevice, SpeakerSetup};
use super::backend::{Backend, BackendEvent, Command, EventSender};
use super::media_options::MediaOptions;
use super::state::PlaybackState;
use super::tracks::{TrackId, TrackKind, Tracks};
use super::values::{BackendSeek, Generation, Offset, Rate, Start, StepDirection, Volume};
use crate::app::media::Media;

/// The media the mock pretends to play.
#[derive(Clone, Debug)]
pub struct MockMedia {
    /// Its length, or `None` for a live stream.
    pub duration: Option<Duration>,
    /// Whether it can seek.
    pub seekable: bool,
    /// Its tracks.
    pub tracks: Tracks,
    /// Whether the mock answers commands with events, or only records them.
    pub responds: bool,
}

impl Default for MockMedia {
    /// A seekable one-minute file that answers every command.
    fn default() -> Self {
        Self {
            duration: Some(Duration::from_secs(60)),
            seekable: true,
            tracks: Tracks::default(),
            responds: true,
        }
    }
}

impl MockMedia {
    /// A mock that only records commands; the test sends every event itself.
    pub fn manual() -> Self {
        Self { responds: false, ..Self::default() }
    }
}

/// What the mock and its handles share.
#[derive(Debug, Default)]
struct Shared {
    generation: Generation,
    commands: Vec<Command>,
}

/// The test's side of a [`MockBackend`].
#[derive(Clone, Debug)]
pub struct MockHandle {
    events: EventSender,
    shared: Arc<Mutex<Shared>>,
}

impl MockHandle {
    /// Sends `event` as the backend, for the latest open.
    pub fn emit(&self, event: BackendEvent) {
        let generation = lock(&self.shared).generation;
        self.events.send(generation, event);
    }

    /// Sends `event` as the backend, for the open with `generation`.
    pub fn emit_for(&self, generation: Generation, event: BackendEvent) {
        self.events.send(generation, event);
    }

    /// The generation of the latest open.
    pub fn generation(&self) -> Generation {
        lock(&self.shared).generation
    }

    /// Every command received so far, oldest first, and forgets them.
    pub fn take_commands(&self) -> Vec<Command> {
        std::mem::take(&mut lock(&self.shared).commands)
    }
}

/// A backend with no media behind it.
pub struct MockBackend {
    events: EventSender,
    media: MockMedia,
    shared: Arc<Mutex<Shared>>,
    shutdown_delay: Duration,
}

impl MockBackend {
    /// A mock pretending to play `media`, and the handle to drive it.
    pub fn new(events: EventSender, media: MockMedia) -> (Self, MockHandle) {
        let shared = Arc::default();
        let handle = MockHandle { events: events.clone(), shared: Arc::clone(&shared) };
        (Self { events, media, shared, shutdown_delay: Duration::ZERO }, handle)
    }

    /// Makes [`Backend::shut_down`] block for `delay`, like a hung network stream.
    pub fn with_shutdown_delay(mut self, delay: Duration) -> Self {
        self.shutdown_delay = delay;
        self
    }

    fn record(&self, command: Command) {
        lock(&self.shared).commands.push(command);
    }

    fn respond(&self, events: impl IntoIterator<Item = BackendEvent>) {
        if !self.media.responds {
            return;
        }
        let generation = lock(&self.shared).generation;
        for event in events {
            self.events.send(generation, event);
        }
    }

    fn opened(&self, start: Start) -> Vec<BackendEvent> {
        let started = if start.paused { PlaybackState::Paused } else { PlaybackState::Playing };
        let position = (!start.at.is_zero()).then_some(BackendEvent::Position(start.at));
        let opening = [
            BackendEvent::State(PlaybackState::Opening),
            BackendEvent::Duration(self.media.duration),
            BackendEvent::Seekable(self.media.seekable),
            BackendEvent::Pausable(true),
            BackendEvent::Tracks(self.media.tracks.clone()),
            BackendEvent::State(PlaybackState::Buffering),
            BackendEvent::Buffering(100.0),
        ];
        opening.into_iter().chain(position).chain([BackendEvent::State(started)]).collect()
    }

    fn seeked(&self, to: BackendSeek) -> Vec<BackendEvent> {
        let duration = self.media.duration.unwrap_or_default();
        let time = match to {
            BackendSeek::Time(time) => time,
            BackendSeek::Fraction(fraction) => duration.mul_f32(fraction.clamp(0.0, 1.0)),
        };
        let mut events = vec![BackendEvent::Position(time), BackendEvent::SeekCompleted];
        if self.media.duration.is_some_and(|duration| time >= duration) {
            events.push(BackendEvent::State(PlaybackState::Ended));
        }
        events
    }
}

impl Backend for MockBackend {
    fn open(&mut self, media: &Media, start: Start, generation: Generation) {
        lock(&self.shared).generation = generation;
        self.record(Command::Open { media: media.clone(), start, generation });
        self.respond(self.opened(start));
    }

    fn play(&mut self) {
        self.record(Command::Play);
        self.respond([BackendEvent::State(PlaybackState::Playing)]);
    }

    fn pause(&mut self) {
        self.record(Command::Pause);
        self.respond([BackendEvent::State(PlaybackState::Paused)]);
    }

    fn stop(&mut self) {
        self.record(Command::Stop);
        self.respond([BackendEvent::State(PlaybackState::Stopped)]);
    }

    fn seek(&mut self, to: BackendSeek) {
        self.record(Command::Seek(to));
        self.respond(self.seeked(to));
    }

    fn frame_step(&mut self, direction: StepDirection) {
        self.record(Command::FrameStep(direction));
        self.respond([BackendEvent::State(PlaybackState::Paused), BackendEvent::FrameStepped]);
    }

    fn set_rate(&mut self, rate: Rate) {
        self.record(Command::SetRate(rate));
    }

    fn set_volume(&mut self, volume: Volume) {
        self.record(Command::SetVolume(volume));
    }

    fn set_mute(&mut self, muted: bool) {
        self.record(Command::SetMute(muted));
    }

    fn select_track(&mut self, kind: TrackKind, track: Option<TrackId>) {
        self.record(Command::SelectTrack(kind, track));
        self.media.tracks.select(kind, track);
        self.respond([BackendEvent::Tracks(self.media.tracks.clone())]);
    }

    fn set_audio_delay(&mut self, delay: Offset) {
        self.record(Command::SetAudioDelay(delay));
    }

    fn set_subtitle_delay(&mut self, delay: Offset) {
        self.record(Command::SetSubtitleDelay(delay));
    }

    fn add_subtitle(&mut self, path: &Path) {
        self.record(Command::AddSubtitle(path.to_path_buf()));
    }

    fn load_subtitle(&mut self, path: &Path) {
        self.record(Command::LoadSubtitle(path.to_path_buf()));
    }

    fn add_audio(&mut self, path: &Path) {
        self.record(Command::AddAudio(path.to_path_buf()));
    }

    fn set_audio_device(&mut self, device: &OutputDevice) {
        self.record(Command::SetAudioDevice(device.clone()));
    }

    fn refresh_audio_devices(&mut self) {
        self.record(Command::RefreshAudioDevices);
    }

    fn set_speaker_setup(&mut self, setup: SpeakerSetup) {
        self.record(Command::SetSpeakerSetup(setup));
    }

    fn set_media_options(&mut self, options: &MediaOptions) {
        self.record(Command::SetMediaOptions(options.clone()));
    }

    fn set_equalizer(&mut self, equalizer: Option<Equalizer>) {
        self.record(Command::SetEqualizer(equalizer));
    }

    fn shut_down(self: Box<Self>) {
        thread::sleep(self.shutdown_delay);
    }
}

fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    // A test that panicked while holding the lock has already failed; keep going for its report.
    shared.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
