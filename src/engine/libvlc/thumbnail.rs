//! The libVLC frame grabber (S4.4.1): a muted libVLC instance and player of its own, created on
//! the thread that first grabs (the thumbnailer's), decoding in software into small BGRA
//! pictures. A grab opens the file at the asked time (`:start-time`), takes the first picture
//! shown, and stops. libVLC decodes the frames before that time without showing them, so the
//! picture is the one at that time.

use std::ffi::{CString, c_void};
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use super::api::{Event, Instance, LibVlc, MediaPlayer};
use super::decoding::SOFTWARE_DECODING;
use super::events::{ENCOUNTERED_ERROR, END_REACHED};
use super::frame_sink::FrameSink;
use super::preview::ARGUMENTS;
use super::vmem::{self, Vmem};
use crate::engine::thumbnail::{Frame, FrameGrabber, Grab, MAX_HEIGHT, MAX_WIDTH};

/// How long one grab may take, opening included, before the file counts as having no picture
/// there.
const GRAB_TIMEOUT: Duration = Duration::from_secs(8);

/// A [`FrameGrabber`] on libVLC. libVLC is loaded the first time it grabs.
#[derive(Default)]
pub struct LibVlcThumbnails {
    state: State,
}

/// Whether the grabber has its player yet.
#[derive(Default)]
enum State {
    /// Nothing grabbed yet.
    #[default]
    Unloaded,
    /// Ready to grab.
    Loaded(Grabber),
    /// libVLC couldn't be loaded (logged once); nothing can be grabbed.
    Failed,
}

impl FrameGrabber for LibVlcThumbnails {
    fn grab(&mut self, path: &Path, at: Duration) -> Grab {
        if matches!(self.state, State::Unloaded) {
            self.state = match Grabber::new() {
                Ok(grabber) => State::Loaded(grabber),
                Err(error) => {
                    tracing::warn!("library: no thumbnails: {error:#}");
                    State::Failed
                }
            };
        }
        match &self.state {
            State::Loaded(grabber) => grabber.grab(path, at).map_or(Grab::Nothing, Grab::Frame),
            State::Unloaded | State::Failed => Grab::Unavailable,
        }
    }
}

/// What the player tells the grabber.
enum Signal {
    /// The sink caught a picture.
    Picture,
    /// The media ended or failed before a picture showed.
    Stopped,
}

/// libVLC, the grabber's instance and player, and what their callbacks point at.
struct Grabber {
    vlc: Arc<LibVlc>,
    instance: *mut Instance,
    player: *mut MediaPlayer,
    sink: Arc<FrameSink>,
    signals: Receiver<Signal>,
    /// Kept alive until the player is released: libVLC's video callbacks point at it.
    vmem: Option<Arc<Vmem>>,
    /// Kept alive until the player is released: the event callback points at it.
    callback: Option<Box<Sender<Signal>>>,
}

// SAFETY: libVLC handles are thread-safe objects, and only the grabber's owner uses them, one
// call at a time.
unsafe impl Send for Grabber {}

impl Grabber {
    fn new() -> anyhow::Result<Self> {
        let vlc = super::library()?;
        let instance = super::new_instance(&vlc, &ARGUMENTS)?;
        // SAFETY: the instance is live.
        let player = unsafe { (vlc.api.libvlc_media_player_new)(instance) };
        if player.is_null() {
            let error = vlc.last_error();
            // SAFETY: the instance is live and released once.
            unsafe { (vlc.api.libvlc_release)(instance) };
            anyhow::bail!("libvlc_media_player_new: {error}");
        }
        let (sender, signals) = mpsc::channel();
        let shown = sender.clone();
        // An error means the grabber is gone; nobody is left to tell.
        let notify = Box::new(move || drop(shown.send(Signal::Picture)));
        let sink = Arc::new(FrameSink::new((MAX_WIDTH, MAX_HEIGHT), notify));
        let vmem = Arc::new(Vmem::new(sink.clone()));
        vmem::install(&vlc, player, &vmem);
        let callback = Box::new(sender);
        let data: *mut c_void = std::ptr::from_ref::<Sender<Signal>>(&callback).cast_mut().cast();
        // SAFETY: the player is live; `data` outlives it (released first in `drop`).
        unsafe {
            let manager = (vlc.api.libvlc_media_player_event_manager)(player);
            for kind in [END_REACHED, ENCOUNTERED_ERROR] {
                (vlc.api.libvlc_event_attach)(manager, kind, on_event, data);
            }
        }
        let (vmem, callback) = (Some(vmem), Some(callback));
        Ok(Self { vlc, instance, player, sink, signals, vmem, callback })
    }

    /// The picture of the file at `path` at `at`, if one shows in time.
    fn grab(&self, path: &Path, at: Duration) -> Option<Frame> {
        while self.signals.try_recv().is_ok() {}
        drop(self.sink.take());
        if !self.start(path, at) {
            return None;
        }
        let frame = self.wait();
        // SAFETY: the player is live; NULL media is allowed.
        unsafe {
            (self.vlc.api.libvlc_media_player_stop)(self.player);
            (self.vlc.api.libvlc_media_player_set_media)(self.player, std::ptr::null_mut());
        }
        frame
    }

    /// Plays the file at `path` from `at`, muted and decoded in software.
    fn start(&self, path: &Path, at: Duration) -> bool {
        let Ok(text) = CString::new(path.to_string_lossy().into_owned()) else { return false };
        let start = format!(":start-time={:.3}", at.as_secs_f64());
        let options = SOFTWARE_DECODING.iter().copied().chain([start.as_str()]);
        let options: Vec<CString> =
            options.map(|option| CString::new(option).expect("no NUL in options")).collect();
        let api = &self.vlc.api;
        // SAFETY: the instance and player are live, every string is NUL-terminated, and the
        // player takes its own reference to the media before ours is released.
        unsafe {
            let media = (api.libvlc_media_new_path)(self.instance, text.as_ptr());
            if media.is_null() {
                return false;
            }
            for option in &options {
                (api.libvlc_media_add_option)(media, option.as_ptr());
            }
            (api.libvlc_media_player_set_media)(self.player, media);
            (api.libvlc_media_release)(media);
            (api.libvlc_media_player_play)(self.player) == 0
        }
    }

    /// Waits for the first picture, until the media stops or [`GRAB_TIMEOUT`] passes.
    fn wait(&self) -> Option<Frame> {
        let deadline = Instant::now() + GRAB_TIMEOUT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.signals.recv_timeout(left) {
                Ok(Signal::Picture) => {}
                Ok(Signal::Stopped) | Err(_) => return None,
            }
            if let Some(frame) = self.sink.take() {
                return Some(frame);
            }
        }
    }
}

impl Drop for Grabber {
    fn drop(&mut self) {
        let api = &self.vlc.api;
        // SAFETY: the handles are live and released once. After the player is released no
        // callback runs, so their data may then be dropped.
        unsafe {
            (api.libvlc_media_player_stop)(self.player);
            (api.libvlc_media_player_release)(self.player);
            (api.libvlc_release)(self.instance);
        }
        drop(self.vmem.take());
        drop(self.callback.take());
    }
}

/// libVLC's event callback for the grabber's player: passes the end and errors on. It never
/// calls libVLC.
unsafe extern "C" fn on_event(event: *const Event, data: *mut c_void) {
    // SAFETY: libVLC passes a live event, and `data` is the boxed sender the grabber owns until
    // after it releases the player.
    let (kind, grabber) = unsafe { ((*event).kind, &*data.cast::<Sender<Signal>>()) };
    if matches!(kind, END_REACHED | ENCOUNTERED_ERROR) {
        // An error means the grabber is being dropped.
        let _ = grabber.send(Signal::Stopped);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    #[ignore = "needs libVLC and the S1.1 test media (see `libvlc/tests.rs`)"]
    fn grabs_a_picture_at_a_time_and_nothing_past_the_end() {
        // SAFETY: the ignored libVLC tests run on one thread (`--test-threads 1`).
        unsafe { crate::engine::libvlc::runtime::prepare_environment() };
        let media = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/spike-media");
        let video = media.join("1080p60-h264.mp4");
        let mut grabber = LibVlcThumbnails::default();
        let Grab::Frame(frame) = grabber.grab(&video, Duration::from_secs(10)) else {
            panic!("the test video has a picture at 10 s");
        };
        // The coded size (1920×1088, padding included) is what gets scaled to fit.
        assert_eq!(frame.height, MAX_HEIGHT);
        assert!(frame.width <= MAX_WIDTH && frame.width > MAX_WIDTH - 16);
        let Grab::Frame(later) = grabber.grab(&video, Duration::from_secs(40)) else {
            panic!("the test video has a picture at 40 s");
        };
        assert_ne!(frame.bgra, later.bgra, "a different time shows a different picture");
        assert_eq!(grabber.grab(&video, Duration::from_secs(600)), Grab::Nothing);
        assert_eq!(grabber.grab(&media.join("audio-only.flac"), Duration::ZERO), Grab::Nothing);
    }
}
