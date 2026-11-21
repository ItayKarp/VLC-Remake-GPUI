//! The libVLC 3 backend (S1.2, ADR 0001).
//!
//! - [`api`] loads libVLC at run time and declares the C API; [`runtime`] finds the libraries.
//! - [`LibVlcBackend`] queues commands for a worker thread ([`worker`], [`control`],
//!   [`query`]) that owns every libVLC handle, so the UI never waits on a libVLC call.
//! - [`events`] is the libVLC event callback: it translates and forwards, and never calls libVLC.
//! - [`failure`] names the cause of an error libVLC reports without one.
//!
//! - [`audio`] lists output devices and picks one (S1.5).
//! - [`vmem`] hands decoded pictures to a [`VideoSink`] (S1.4.1), and [`prepare`] reads a file's
//!   video geometry before it plays. Without a sink, libVLC's dummy output decodes and discards
//!   pictures, so libVLC never opens a window of its own.

mod api;
mod audio;
mod control;
mod decoding;
mod events;
mod failure;
mod prepare;
mod query;
pub mod runtime;
mod size_watch;
#[cfg(test)]
mod tests;
mod track_list;
#[cfg(test)]
mod video_tests;
mod vmem;
mod worker;

use std::ffi::CString;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::thread::{self, JoinHandle};

use anyhow::{Context as _, Result, anyhow, bail};

use self::api::{Instance, LibVlc};
use self::events::Shared;
use self::runtime::{PLUGIN_PATH_ENV, Runtime};
use self::vmem::Vmem;
use self::worker::{Handles, Message, Worker};
use super::audio::{AudioConfig, OutputDevice, SpeakerSetup};
use super::backend::{Backend, Command, EventSender};
use super::tracks::{TrackId, TrackKind};
use super::values::{BackendSeek, Generation, Offset, Rate, StepDirection, Volume};
use super::video::VideoSink;
use crate::app::media::Media;

/// The arguments every libVLC instance starts with: the file name would otherwise be drawn into
/// the video when it starts.
const ARGUMENTS: [&str; 1] = ["--no-video-title-show"];
/// Added without a video sink: decode and discard video.
const NO_VIDEO_ARGUMENT: &str = "--vout=vdummy";

/// The libVLC library, loaded once per process. A failure is kept as text so every caller
/// gets the same answer without retrying.
fn library() -> Result<Arc<LibVlc>> {
    static LIBRARY: OnceLock<Result<Arc<LibVlc>, String>> = OnceLock::new();
    LIBRARY
        .get_or_init(|| {
            let runtime = Runtime::located();
            if runtime.plugin_dir.is_some() && std::env::var_os(PLUGIN_PATH_ENV).is_none() {
                tracing::warn!("libvlc: {PLUGIN_PATH_ENV} isn't set; libVLC may find no plugins");
            }
            let vlc = LibVlc::load(runtime).map_err(|error| format!("{error:#}"))?;
            tracing::info!("libvlc: loaded {} from {:?}", vlc.version(), runtime.lib_dir);
            Ok(Arc::new(vlc))
        })
        .clone()
        .map_err(|error| anyhow!(error))
}

/// A [`Backend`] playing through libVLC 3.
pub struct LibVlcBackend {
    worker: mpsc::Sender<Message>,
    thread: Option<JoinHandle<()>>,
}

impl LibVlcBackend {
    /// Loads libVLC (once per process), creates an instance set up with `audio` and a media
    /// player, and starts the worker thread that will report through `events` and send
    /// pictures to `video` (or discard them, with `None`).
    pub fn new(
        events: EventSender,
        video: Option<Arc<dyn VideoSink>>,
        audio: AudioConfig,
    ) -> Result<Self> {
        let vlc = library()?;
        let handles = new_handles(&vlc, video.is_some(), audio)?;
        let video = video.map(|sink| Arc::new(Vmem::new(sink)));
        if let Some(vmem) = &video {
            vmem::install(&vlc, handles.player, vmem);
        }
        let (sender, inbox) = mpsc::channel();
        let shared = Arc::new(Shared {
            events,
            worker: sender.clone(),
            generation: Mutex::new(Generation::default()),
            started: AtomicBool::new(false),
            paused: AtomicBool::new(false),
            seeking: AtomicBool::new(false),
            switching: AtomicBool::new(false),
        });
        let thread = thread::Builder::new()
            .name("libvlc-worker".into())
            .spawn(move || Worker::new(vlc, handles, shared, video).run(&inbox))
            .context("starting the libVLC worker thread")?;
        Ok(Self { worker: sender, thread: Some(thread) })
    }

    fn send(&self, command: Command) {
        // An error means the worker is gone (shut down); commands are then ignored.
        let _ = self.worker.send(Message::Command(command));
    }
}

impl Backend for LibVlcBackend {
    fn open(&mut self, media: &Media, generation: Generation) {
        self.send(Command::Open { media: media.clone(), generation });
    }

    fn play(&mut self) {
        self.send(Command::Play);
    }

    fn pause(&mut self) {
        self.send(Command::Pause);
    }

    fn stop(&mut self) {
        self.send(Command::Stop);
    }

    fn seek(&mut self, to: BackendSeek) {
        self.send(Command::Seek(to));
    }

    fn frame_step(&mut self, direction: StepDirection) {
        self.send(Command::FrameStep(direction));
    }

    fn set_rate(&mut self, rate: Rate) {
        self.send(Command::SetRate(rate));
    }

    fn set_volume(&mut self, volume: Volume) {
        self.send(Command::SetVolume(volume));
    }

    fn set_mute(&mut self, muted: bool) {
        self.send(Command::SetMute(muted));
    }

    fn select_track(&mut self, kind: TrackKind, track: Option<TrackId>) {
        self.send(Command::SelectTrack(kind, track));
    }

    fn set_audio_delay(&mut self, delay: Offset) {
        self.send(Command::SetAudioDelay(delay));
    }

    fn set_subtitle_delay(&mut self, delay: Offset) {
        self.send(Command::SetSubtitleDelay(delay));
    }

    fn add_subtitle(&mut self, path: &Path) {
        self.send(Command::AddSubtitle(path.to_path_buf()));
    }

    fn set_audio_device(&mut self, device: &OutputDevice) {
        self.send(Command::SetAudioDevice(device.clone()));
    }

    fn set_speaker_setup(&mut self, setup: SpeakerSetup) {
        self.send(Command::SetSpeakerSetup(setup));
    }

    fn shut_down(mut self: Box<Self>) {
        let _ = self.worker.send(Message::Shutdown);
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::error!("libvlc: the worker thread panicked");
        }
    }
}

impl Drop for LibVlcBackend {
    /// Without [`Backend::shut_down`], the worker still releases libVLC, just unwaited for.
    fn drop(&mut self) {
        if self.thread.is_some() {
            let _ = self.worker.send(Message::Shutdown);
        }
    }
}

/// A new libVLC instance with [`ARGUMENTS`] and `audio`'s arguments, and a media player on it;
/// `with_video` false adds [`NO_VIDEO_ARGUMENT`]. An audio argument this libVLC doesn't know
/// fails the instance, so it is retried without them.
fn new_handles(vlc: &LibVlc, with_video: bool, audio: AudioConfig) -> Result<Handles> {
    let video = (!with_video).then_some(NO_VIDEO_ARGUMENT);
    let base: Vec<&str> = ARGUMENTS.into_iter().chain(video).collect();
    let audio_arguments = audio::arguments(audio);
    let instance = if audio_arguments.is_empty() {
        new_instance(vlc, &base)?
    } else {
        let full: Vec<&str> = base.iter().copied().chain(audio_arguments).collect();
        new_instance(vlc, &full).or_else(|error| {
            tracing::warn!("libvlc: audio options refused ({error:#}); starting without them");
            new_instance(vlc, &base)
        })?
    };
    // SAFETY: the instance is live.
    let player = unsafe { (vlc.api.libvlc_media_player_new)(instance) };
    if player.is_null() {
        let error = vlc.last_error();
        // SAFETY: the instance is live and released once.
        unsafe { (vlc.api.libvlc_release)(instance) };
        bail!("libvlc_media_player_new: {error}");
    }
    Ok(Handles { instance, player })
}

/// A new libVLC instance started with `arguments`.
fn new_instance(vlc: &LibVlc, arguments: &[&str]) -> Result<*mut Instance> {
    let arguments: Vec<CString> = arguments
        .iter()
        .map(|argument| CString::new(*argument).expect("no NUL in constants"))
        .collect();
    let pointers: Vec<_> = arguments.iter().map(|argument| argument.as_ptr()).collect();
    // SAFETY: `pointers` holds NUL-terminated strings that outlive the call.
    let instance: *mut Instance =
        unsafe { (vlc.api.libvlc_new)(pointers.len() as i32, pointers.as_ptr()) };
    if instance.is_null() {
        bail!("libvlc_new: {}", vlc.last_error());
    }
    Ok(instance)
}
