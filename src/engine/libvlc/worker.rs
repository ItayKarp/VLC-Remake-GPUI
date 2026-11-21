//! The backend's worker thread: the only thread that calls libVLC after setup. Commands from the
//! player and follow-up work from the event callback queue here, so slow libVLC calls (`stop` and
//! `set_media` join libVLC's input thread, which a hung network source can stall) never block
//! the UI, and nothing calls libVLC from inside its own callbacks.

use std::ffi::CString;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, PoisonError};
use std::time::{Duration, Instant};

use super::api::{EventManager, Instance, LibVlc, Media, MediaPlayer};
use super::audio::DeviceWatch;
use super::events::{self, MEDIA_META_CHANGED, PLAYER_EVENTS, Shared};
use super::failure;
use super::size_watch::{SIZE_POLL, SizeWatch};
use super::vmem::Vmem;
use crate::app::media::Media as Source;
use crate::engine::audio::SpeakerSetup;
use crate::engine::backend::{BackendEvent, Command};
use crate::engine::state::{ErrorKind, PlaybackState};
use crate::engine::values::{Generation, Offset};

/// How long a seek may wait for libVLC to report the new time before it counts as done anyway
/// (libVLC 3 has no "seek finished" event, and a paused or broken input may never report).
const SEEK_SETTLE: Duration = Duration::from_millis(300);

/// Which part of the media's description to read again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refresh {
    /// Tracks, selection and video size.
    Tracks,
    /// The chapter list.
    Chapters,
    /// The metadata tags.
    Metadata,
    /// All of it, once playback started; also re-applies per-media settings.
    Everything,
    /// The output device list (the output switched devices).
    AudioDevices,
}

/// Work for the worker thread.
#[derive(Debug)]
pub enum Message {
    /// A command from the player.
    Command(Command),
    /// The callback saw something that needs libVLC queries to describe.
    Refresh(Refresh),
    /// libVLC reached the end of the media.
    Ended,
    /// libVLC reported an error, which the worker classifies.
    Failed,
    /// Stop, release everything, and exit.
    Shutdown,
}

/// The libVLC handles the worker owns. libVLC objects may be used from any thread; after setup
/// only the worker uses these.
pub struct Handles {
    /// The libVLC instance.
    pub instance: *mut Instance,
    /// The one media player, reused for every open.
    pub player: *mut MediaPlayer,
}

// SAFETY: libVLC handles are thread-safe objects; ownership moves to the worker thread whole.
unsafe impl Send for Handles {}

/// The worker's state.
pub struct Worker {
    pub(super) vlc: Arc<LibVlc>,
    pub(super) handles: Handles,
    pub(super) shared: Arc<Shared>,
    /// The media libVLC is playing, holding our reference to it.
    pub(super) media: *mut Media,
    pub(super) source: Option<Source>,
    pub(super) audio_delay: Offset,
    pub(super) subtitle_delay: Offset,
    /// Where video goes; `None` decodes and discards it.
    pub(super) video: Option<Arc<Vmem>>,
    pub(super) size_watch: SizeWatch,
    /// The forced speaker setup, re-applied to each media.
    pub(super) speaker_setup: SpeakerSetup,
    /// When the output device list is read next.
    pub(super) devices: DeviceWatch,
}

impl Worker {
    /// A worker for `handles`, sending video to `video`; call [`Self::run`] on its own thread.
    pub fn new(
        vlc: Arc<LibVlc>,
        handles: Handles,
        shared: Arc<Shared>,
        video: Option<Arc<Vmem>>,
    ) -> Self {
        let worker = Self {
            vlc,
            handles,
            shared,
            media: std::ptr::null_mut(),
            source: None,
            audio_delay: Offset::default(),
            subtitle_delay: Offset::default(),
            video,
            size_watch: SizeWatch::default(),
            speaker_setup: SpeakerSetup::Automatic,
            devices: DeviceWatch::default(),
        };
        worker.attach(worker.player_events(), &PLAYER_EVENTS);
        worker
    }

    /// Handles messages until [`Message::Shutdown`], then releases everything.
    pub fn run(mut self, inbox: &Receiver<Message>) {
        while let Some(message) = self.next_message(inbox) {
            match message {
                Message::Command(command) => self.execute(command),
                Message::Refresh(what) => self.refresh(what),
                Message::Ended => self.ended(),
                Message::Failed => self.failed(),
                Message::Shutdown => break,
            }
            // A steady stream of messages must not starve the device list.
            self.poll_devices_if_due();
        }
        self.release();
    }

    /// The next message. Meanwhile it settles a seek libVLC never confirmed after
    /// [`SEEK_SETTLE`] without messages, reads a still-unknown video size every [`SIZE_POLL`],
    /// and reads the device list when due. `None` once every sender is gone.
    fn next_message(&mut self, inbox: &Receiver<Message>) -> Option<Message> {
        let idle_since = Instant::now();
        loop {
            match inbox.recv_timeout(self.idle_wait(idle_since)) {
                Ok(message) => return Some(message),
                Err(RecvTimeoutError::Disconnected) => return None,
                Err(RecvTimeoutError::Timeout) => self.idle(idle_since),
            }
        }
    }

    /// How long to wait for a message before there is idle work to do.
    fn idle_wait(&self, idle_since: Instant) -> Duration {
        let now = Instant::now();
        let mut wait = self.devices.until_poll(now);
        if self.size_watch.is_watching(now) {
            wait = wait.min(SIZE_POLL);
        }
        if self.shared.seeking.load(Ordering::Relaxed) {
            wait = wait.min(SEEK_SETTLE.saturating_sub(idle_since.elapsed()));
        }
        wait
    }

    /// Replaces the media with `source` and starts playing it as open `generation`.
    pub(super) fn open(&mut self, source: Source, generation: Generation) {
        let media = self.new_media(&source);
        if !media.is_null() {
            self.prepare_video(media, &source);
        }
        // `set_media` stops the previous media first; its events still carry the old
        // generation, because the new one is published only afterwards.
        // SAFETY: the player is live; `media` is NULL or live, and the player takes its own
        // reference.
        unsafe { (self.vlc.api.libvlc_media_player_set_media)(self.handles.player, media) };
        self.release_media();
        self.media = media;
        self.source = Some(source);
        self.size_watch.stop();
        *self.shared.generation.lock().unwrap_or_else(PoisonError::into_inner) = generation;
        for flag in [&self.shared.started, &self.shared.paused, &self.shared.seeking] {
            flag.store(false, Ordering::Relaxed);
        }
        if media.is_null() {
            tracing::warn!("libvlc: couldn't create the media: {}", self.vlc.last_error());
            self.shared.send(BackendEvent::State(PlaybackState::Error(ErrorKind::OpenFailed)));
            return;
        }
        self.attach(self.media_events(), &[MEDIA_META_CHANGED]);
        // SAFETY: the player is live.
        if unsafe { (self.vlc.api.libvlc_media_player_play)(self.handles.player) } != 0 {
            tracing::warn!("libvlc: play failed: {}", self.vlc.last_error());
            self.shared.send(BackendEvent::State(PlaybackState::Error(ErrorKind::OpenFailed)));
        }
    }

    fn new_media(&self, source: &Source) -> *mut Media {
        let (text, from_path) = match source {
            Source::File(path) => (path.to_string_lossy().into_owned(), true),
            Source::Url(url) => (url.clone(), false),
        };
        let Ok(text) = CString::new(text) else { return std::ptr::null_mut() };
        let api = &self.vlc.api;
        let create =
            if from_path { api.libvlc_media_new_path } else { api.libvlc_media_new_location };
        // SAFETY: the instance is live and `text` is NUL-terminated.
        unsafe { create(self.handles.instance, text.as_ptr()) }
    }

    /// Nothing arrived since `idle_since`: reads the device list when due, re-reads a
    /// still-unknown video size, and settles a seek that waited [`SEEK_SETTLE`].
    fn idle(&mut self, idle_since: Instant) {
        self.poll_devices_if_due();
        if self.size_watch.is_watching(Instant::now()) {
            self.send_video_size();
        }
        if idle_since.elapsed() >= SEEK_SETTLE {
            self.settle_seek();
        }
    }

    fn settle_seek(&self) {
        if self.shared.seeking.swap(false, Ordering::Relaxed) {
            self.shared.send(BackendEvent::SeekCompleted);
        }
    }

    fn ended(&self) {
        let state = if self.track_count() == 0 {
            PlaybackState::Error(ErrorKind::NoPlayableStreams)
        } else {
            PlaybackState::Ended
        };
        self.shared.send(BackendEvent::State(state));
    }

    fn failed(&self) {
        let started = self.shared.started.load(Ordering::Relaxed);
        let kind = failure::classify(self.source.as_ref(), started);
        tracing::warn!("libvlc: playback failed ({kind:?}): {}", self.vlc.last_error());
        self.shared.send(BackendEvent::State(PlaybackState::Error(kind)));
    }

    fn player_events(&self) -> *mut EventManager {
        // SAFETY: the player is live; its event manager lives as long as it does.
        unsafe { (self.vlc.api.libvlc_media_player_event_manager)(self.handles.player) }
    }

    fn media_events(&self) -> *mut EventManager {
        // SAFETY: the media is live; its event manager lives as long as it does.
        unsafe { (self.vlc.api.libvlc_media_event_manager)(self.media) }
    }

    fn attach(&self, manager: *mut EventManager, kinds: &[i32]) {
        let data = Arc::as_ptr(&self.shared).cast_mut().cast();
        for &kind in kinds {
            // SAFETY: the manager is live, and `data` outlives it (released in `release`).
            unsafe { (self.vlc.api.libvlc_event_attach)(manager, kind, events::on_event, data) };
        }
    }

    fn release_media(&mut self) {
        if !self.media.is_null() {
            // SAFETY: we hold one reference to the media, released once here.
            unsafe { (self.vlc.api.libvlc_media_release)(self.media) };
            self.media = std::ptr::null_mut();
        }
    }

    fn release(mut self) {
        let api = &self.vlc.api;
        // SAFETY: the handles are live and released once. `stop` joins libVLC's threads, and
        // after the player is released no callback can run, so `shared` may then be dropped.
        unsafe {
            (api.libvlc_media_player_stop)(self.handles.player);
            (api.libvlc_media_player_release)(self.handles.player);
        }
        self.release_media();
        // SAFETY: nothing uses the instance any more.
        unsafe { (self.vlc.api.libvlc_release)(self.handles.instance) };
        // The player is gone, so `vmem`'s callbacks can't run: the sink may be dropped now.
        drop(self.video.take());
    }
}
