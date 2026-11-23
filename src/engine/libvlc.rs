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
mod audio_effects;
mod certificates;
mod color_filters;
#[cfg(test)]
mod color_tests;
mod control;
mod core_vars;
mod cpu_filters;
mod decoding;
mod enhance;
#[cfg(test)]
mod enhance_tests;
mod equalizer;
mod events;
mod failure;
mod frame_sink;
pub mod hardware;
mod hold;
pub mod lengths;
mod media_options;
pub mod options;
mod picture;
mod prepare;
pub mod preview;
mod query;
mod reopen;
#[cfg(test)]
mod reopen_tests;
pub mod runtime;
mod size_watch;
mod subtitle_style;
pub mod system_options;
pub mod tags;
#[cfg(test)]
mod tests;
pub mod thumbnail;
mod track_list;
mod video_filters;
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
use super::audio::equalizer::Equalizer;
use super::audio::{AudioConfig, OutputDevice, SpeakerSetup};
use super::backend::{Backend, Command, EventSender};
use super::colors::Colors;
use super::media_options::{MediaOptions, Picture, VideoFilters};
use super::subtitle_style::SubtitleStyle;
use super::tracks::{TrackId, TrackKind};
use super::values::{BackendSeek, Generation, Offset, Rate, Start, StepDirection, Volume};
use super::video::VideoSink;
use crate::app::media::Media;

/// The arguments every libVLC instance starts with: the file name would otherwise be drawn into
/// the video when it starts, and the app finds subtitle files beside the video itself
/// ([`super::sidecar`]) to know which track came from which file.
const ARGUMENTS: [&str; 2] = ["--no-video-title-show", "--no-sub-autodetect-file"];
/// Added without a video sink: decode and discard video.
const NO_VIDEO_ARGUMENT: &str = "--vout=vdummy";

/// The libVLC library once something asked for it, or why it couldn't be loaded.
static LIBRARY: OnceLock<Result<Arc<LibVlc>, String>> = OnceLock::new();

/// The libVLC library, loaded once per process. A failure is kept as text so every caller
/// gets the same answer without retrying.
fn library() -> Result<Arc<LibVlc>> {
    LIBRARY.get_or_init(load).clone().map_err(|error| anyhow!(error))
}

/// Loads the libVLC this build runs with, or says why it couldn't.
fn load() -> std::result::Result<Arc<LibVlc>, String> {
    let runtime = Runtime::located();
    warn_without_plugin_path(runtime);
    let vlc = LibVlc::load(runtime).map_err(|error| format!("{error:#}"))?;
    tracing::info!("libvlc: loaded {} from {:?}", vlc.version(), runtime.lib_dir);
    Ok(Arc::new(vlc))
}

/// Warns when `runtime` has its own plugins but libVLC hasn't been told where they are.
fn warn_without_plugin_path(runtime: &Runtime) {
    if runtime.plugin_dir.is_some() && std::env::var_os(PLUGIN_PATH_ENV).is_none() {
        tracing::warn!("libvlc: {PLUGIN_PATH_ENV} isn't set; libVLC may find no plugins");
    }
}

/// The libVLC version string (`3.0.23 Vetinari`), loading the library if nothing has yet, or
/// `None` when it can't be loaded.
pub fn version() -> Option<String> {
    library().ok().map(|vlc| vlc.version())
}

/// The libVLC version string if the library is already loaded; never loads it (crash reports).
pub fn loaded_version() -> Option<String> {
    LIBRARY.get()?.as_ref().ok().map(|vlc| vlc.version())
}

/// A [`Backend`] playing through libVLC 3.
pub struct LibVlcBackend {
    worker: mpsc::Sender<Message>,
    thread: Option<JoinHandle<()>>,
}

impl LibVlcBackend {
    /// Loads libVLC (once per process), creates an instance set up with `audio` and the
    /// `subtitles` style and a media player, and starts the worker thread that will report through `events` and send
    /// pictures to `video` (or discard them, with `None`).
    pub fn new(
        events: EventSender,
        video: Option<Arc<dyn VideoSink>>,
        audio: AudioConfig,
        subtitles: &SubtitleStyle,
    ) -> Result<Self> {
        let vlc = library()?;
        let mut setup = audio::arguments(audio);
        setup.extend(subtitle_style::arguments(subtitles));
        let handles = new_handles(&vlc, video.is_some(), &setup)?;
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
            holding: AtomicBool::new(false),
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
    fn open(&mut self, media: &Media, start: Start, generation: Generation) {
        self.send(Command::Open { media: media.clone(), start, generation });
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

    fn load_subtitle(&mut self, path: &Path) {
        self.send(Command::LoadSubtitle(path.to_path_buf()));
    }

    fn add_audio(&mut self, path: &Path) {
        self.send(Command::AddAudio(path.to_path_buf()));
    }

    fn set_audio_device(&mut self, device: &OutputDevice) {
        self.send(Command::SetAudioDevice(device.clone()));
    }

    fn refresh_audio_devices(&mut self) {
        self.send(Command::RefreshAudioDevices);
    }

    fn set_speaker_setup(&mut self, setup: SpeakerSetup) {
        self.send(Command::SetSpeakerSetup(setup));
    }

    fn set_media_options(&mut self, options: &MediaOptions) {
        self.send(Command::SetMediaOptions(options.clone()));
    }

    fn set_equalizer(&mut self, equalizer: Option<Equalizer>) {
        self.send(Command::SetEqualizer(equalizer));
    }

    fn set_picture(&mut self, picture: Option<Picture>) {
        self.send(Command::SetPicture(picture));
    }

    fn set_video_filters(&mut self, filters: VideoFilters) {
        self.send(Command::SetVideoFilters(filters));
    }

    fn set_colors(&mut self, colors: Colors) {
        self.send(Command::SetColors(colors));
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

/// A new libVLC instance with [`ARGUMENTS`], the `setup` arguments (audio and subtitle style),
/// the Network and Performance settings ([`system_options`]) and the user's startup options
/// ([`options`]), and a media player on it; `with_video` false adds [`NO_VIDEO_ARGUMENT`].
fn new_handles(vlc: &LibVlc, with_video: bool, setup: &[String]) -> Result<Handles> {
    let video = (!with_video).then_some(NO_VIDEO_ARGUMENT);
    let system = system_options::arguments(&system_options::current());
    let groups: [(&str, &[String]); 3] =
        [("audio", setup), ("network and performance", &system), ("startup", options::startup())];
    let instance = start_instance(vlc, video, &groups)?;
    if !system_options::current().verify_certificates {
        certificates::install(vlc, instance);
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

/// A new libVLC instance with every group of options. An option this libVLC doesn't know fails
/// the instance, so it is retried without the last group, then the one before, and so on.
fn start_instance(
    vlc: &LibVlc,
    video: Option<&str>,
    groups: &[(&str, &[String])],
) -> Result<*mut Instance> {
    let mut count = groups.len();
    loop {
        let options = groups[..count].iter().flat_map(|(_, options)| options.iter());
        let arguments: Vec<&str> =
            ARGUMENTS.into_iter().chain(video).chain(options.map(String::as_str)).collect();
        let error = match new_instance(vlc, &arguments) {
            Ok(instance) => return Ok(instance),
            Err(error) => error,
        };
        let Some(last) = groups[..count].iter().rposition(|(_, options)| !options.is_empty())
        else {
            return Err(error);
        };
        tracing::warn!(
            "libvlc: {} options refused ({error:#}); starting without them",
            groups[last].0
        );
        count = last;
    }
}

/// A new libVLC instance started with `arguments`.
fn new_instance(vlc: &LibVlc, arguments: &[&str]) -> Result<*mut Instance> {
    let arguments: Vec<CString> = arguments
        .iter()
        .map(|argument| CString::new(*argument))
        .collect::<Result<_, _>>()
        .context("an option holds a NUL character")?;
    let pointers: Vec<_> = arguments.iter().map(|argument| argument.as_ptr()).collect();
    // SAFETY: `pointers` holds NUL-terminated strings that outlive the call.
    let instance: *mut Instance =
        unsafe { (vlc.api.libvlc_new)(pointers.len() as i32, pointers.as_ptr()) };
    if instance.is_null() {
        bail!("libvlc_new: {}", vlc.last_error());
    }
    Ok(instance)
}
