//! Converting media to another format (S8.10.2), each conversion on a thread of its own with a
//! libVLC instance of its own (`libvlc::transcode`), so playback goes on untouched.
//!
//! The output file is made empty first, refusing a name that exists, so nothing is written
//! over. A conversion that is cancelled or fails leaves no file behind. One that runs the disk
//! down to [`recording::STOP_SPACE`] stops, so the disk isn't filled to the last byte.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use futures::channel::mpsc;

use super::libvlc::transcode::{self, Ending, Streams};
use crate::app::media::Media;
use crate::recording;

/// One conversion to make.
#[derive(Clone, Debug)]
pub struct Request {
    /// What it converts.
    pub source: Media,
    /// The libVLC media options that make the format, writing to `output`.
    pub options: Vec<String>,
    /// The file it writes.
    pub output: PathBuf,
    /// Whether the format keeps only sound, so a source without any has nothing to give.
    pub audio_only: bool,
}

/// Why a conversion didn't finish.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The output would be the source itself.
    SameAsSource,
    /// The output's folder can't be written; the OS's words.
    Unwritable(String),
    /// The disk ran out of room.
    DiskFull,
    /// The source has no stream the format keeps (no sound for an audio format), or none that
    /// could be decoded.
    NothingToConvert {
        /// Whether the format keeps only sound.
        audio_only: bool,
    },
    /// libVLC couldn't read or encode it; why, in its words where it gave any.
    Failed(String),
}

impl Failure {
    /// What the queue says about it.
    pub fn message(&self) -> String {
        match self {
            Self::SameAsSource => tr!("It would replace the file it converts").into(),
            Self::Unwritable(why) => tr!("The folder can't be written to: {why}", why),
            Self::DiskFull => tr!("The disk is full").into(),
            Self::NothingToConvert { audio_only: true } => tr!("It has no sound to convert").into(),
            Self::NothingToConvert { audio_only: false } => {
                tr!("It has no sound or picture that can be converted").into()
            }
            Self::Failed(why) => why.clone(),
        }
    }
}

/// What a [`ConversionJob`] reports.
#[derive(Clone, Debug, PartialEq)]
pub enum ConversionEvent {
    /// How far it got, 0 to 1.
    Progress(f32),
    /// It ended: the file is complete, or it failed and left nothing.
    Finished(Result<(), Failure>),
    /// It was cancelled and left nothing.
    Cancelled,
}

/// The receiving end of a job's reports.
pub type ConversionReceiver = mpsc::UnboundedReceiver<ConversionEvent>;

/// A conversion running on a thread of its own; dropping it cancels it.
#[derive(Debug)]
pub struct ConversionJob {
    cancelled: Arc<AtomicBool>,
}

impl ConversionJob {
    /// Starts `request` on a thread of its own.
    pub fn start(request: Request) -> (Self, ConversionReceiver) {
        let (sender, receiver) = mpsc::unbounded();
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let failed = sender.clone();
        let started = thread::Builder::new()
            .name("conversion".into())
            .spawn(move || _ = sender.unbounded_send(run(&request, &flag, &sender)));
        if let Err(error) = started {
            let failure = Failure::Failed(tr!("couldn't start: {error}", error));
            _ = failed.unbounded_send(ConversionEvent::Finished(Err(failure)));
        }
        (Self { cancelled }, receiver)
    }

    /// Stops it; its file is removed, and it reports [`ConversionEvent::Cancelled`].
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

impl Drop for ConversionJob {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// Makes `request`, reporting progress through `events`, and says how it ended. Whatever didn't
/// finish is removed.
fn run(
    request: &Request,
    cancelled: &AtomicBool,
    events: &mpsc::UnboundedSender<ConversionEvent>,
) -> ConversionEvent {
    if let Err(failure) = claim(request) {
        return ConversionEvent::Finished(Err(failure));
    }
    let folder = request.output.parent().unwrap_or(Path::new("."));
    let mut full = false;
    let tick = |fraction| {
        _ = events.unbounded_send(ConversionEvent::Progress(fraction));
        full = !recording::has_room(folder, recording::STOP_SPACE);
        !full && !cancelled.load(Ordering::Relaxed)
    };
    let ended = transcode::transcode(&request.source, &request.options, tick);
    let event = match ended {
        Ok(Ending::Finished(streams)) => finished(request, streams),
        Ok(Ending::Stopped) if full => ConversionEvent::Finished(Err(Failure::DiskFull)),
        Ok(Ending::Stopped) => ConversionEvent::Cancelled,
        Err(error) => ConversionEvent::Finished(Err(Failure::Failed(format!("{error:#}")))),
    };
    if event != ConversionEvent::Finished(Ok(())) {
        remove(&request.output);
    }
    event
}

/// Makes the output file, empty, refusing the source itself, a name that exists and a disk
/// without room to start.
fn claim(request: &Request) -> Result<(), Failure> {
    if crate::convert::is_source(&request.source, &request.output) {
        return Err(Failure::SameAsSource);
    }
    let folder = request.output.parent().unwrap_or(Path::new("."));
    if !recording::has_room(folder, recording::START_SPACE) {
        return Err(Failure::DiskFull);
    }
    let made = fs::OpenOptions::new().write(true).create_new(true).open(&request.output);
    made.map(drop).map_err(|error| match error.kind() {
        io::ErrorKind::AlreadyExists => Failure::Unwritable(tr!(
            "{name} appeared there meanwhile",
            name = request
                .output
                .file_name()
                .map_or_default(|name| name.to_string_lossy().into_owned())
        )),
        _ => Failure::Unwritable(error.to_string()),
    })
}

/// What a conversion that went through the whole source came to: nothing, when the source had
/// no stream the format keeps.
fn finished(request: &Request, streams: Streams) -> ConversionEvent {
    let kept = if request.audio_only { streams.audio } else { streams.audio || streams.video };
    let written = fs::metadata(&request.output).is_ok_and(|file| file.len() > 0);
    if kept && written {
        ConversionEvent::Finished(Ok(()))
    } else {
        ConversionEvent::Finished(Err(Failure::NothingToConvert { audio_only: request.audio_only }))
    }
}

/// Removes the unfinished file at `path`.
fn remove(path: &Path) {
    if let Err(error) = fs::remove_file(path)
        && error.kind() != io::ErrorKind::NotFound
    {
        tracing::warn!("convert: couldn't remove the unfinished {}: {error}", path.display());
    }
}
