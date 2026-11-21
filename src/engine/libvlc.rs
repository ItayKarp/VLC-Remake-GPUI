//! The libVLC 3 backend (S1.2, ADR 0001).
//!
//! - [`api`] loads libVLC at run time and declares the C API; [`runtime`] finds the libraries.
//! - [`LibVlcBackend`] queues commands for a worker thread ([`worker`], [`control`],
//!   [`query`]) that owns every libVLC handle, so the UI never waits on a libVLC call.
//! - [`events`] is the libVLC event callback: it translates and forwards, and never calls libVLC.
//! - [`failure`] names the cause of an error libVLC reports without one.
//!
//! Video output is a placeholder until S1.4 attaches the presentation path proven in S1.1: the
//! dummy video output decodes and discards pictures, so libVLC never opens a window of its own.

mod api;
mod control;
mod events;
mod failure;
mod query;
pub mod runtime;
#[cfg(test)]
mod tests;
mod worker;

use std::ffi::CString;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::thread::{self, JoinHandle};

use anyhow::{Context as _, Result, anyhow, bail};

use self::api::{Instance, LibVlc};
use self::events::Shared;
use self::runtime::{PLUGIN_PATH_ENV, Runtime};
use self::worker::{Handles, Message, Worker};
use super::backend::{Backend, Command, EventSender};
use super::media_info::{TrackId, TrackKind};
use super::values::{BackendSeek, Generation, Offset, Rate, StepDirection, Volume};
use crate::app::media::Media;

/// The arguments every libVLC instance starts with.
const ARGUMENTS: [&str; 2] = [
    // The file name would be drawn into the video when it starts.
    "--no-video-title-show",
    // Decode and discard video until S1.4 attaches the real output.
    "--vout=vdummy",
];

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
    /// Loads libVLC (once per process), creates an instance and a media player, and starts the
    /// worker thread that will report through `events`.
    pub fn new(events: EventSender) -> Result<Self> {
        let vlc = library()?;
        let handles = new_handles(&vlc)?;
        let (sender, inbox) = mpsc::channel();
        let shared = Arc::new(Shared {
            events,
            worker: sender.clone(),
            generation: Mutex::new(Generation::default()),
            started: AtomicBool::new(false),
            paused: AtomicBool::new(false),
            seeking: AtomicBool::new(false),
        });
        let thread = thread::Builder::new()
            .name("libvlc-worker".into())
            .spawn(move || Worker::new(vlc, handles, shared).run(&inbox))
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

/// A new libVLC instance with [`ARGUMENTS`] and a media player on it.
fn new_handles(vlc: &LibVlc) -> Result<Handles> {
    let arguments = ARGUMENTS.map(|argument| CString::new(argument).expect("no NUL in constants"));
    let pointers = arguments.each_ref().map(|argument| argument.as_ptr());
    // SAFETY: `pointers` holds NUL-terminated strings that outlive the call.
    let instance: *mut Instance =
        unsafe { (vlc.api.libvlc_new)(pointers.len() as i32, pointers.as_ptr()) };
    if instance.is_null() {
        bail!("libvlc_new: {}", vlc.last_error());
    }
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
