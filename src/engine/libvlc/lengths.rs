//! The libVLC length probe (S3.2.2): a libVLC instance of its own and a worker thread that
//! parses each local file's headers and reads its duration. Network media isn't parsed (it
//! could take seconds and has no length when live); its length is learned when it plays.

use std::collections::VecDeque;
use std::ffi::CString;
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{Context as _, Result};

use super::api::{Instance, LibVlc};
use super::prepare::parse_headers;
use crate::app::media::Media;
use crate::engine::lengths::{LengthProbe, LengthSender};

/// The probe instance's arguments: it never plays, so it needs no outputs.
const ARGUMENTS: [&str; 3] = ["--no-audio", "--no-video", "--no-sub-autodetect-file"];
/// How long one file may take to parse before its length counts as unknown.
const PARSE_TIMEOUT: Duration = Duration::from_secs(5);

/// Work for the probe's worker.
enum Message {
    Probe(Vec<Media>),
    Clear,
    Shutdown,
}

/// A [`LengthProbe`] on libVLC.
pub struct LibVlcLengths {
    worker: mpsc::Sender<Message>,
    thread: Option<JoinHandle<()>>,
}

impl LibVlcLengths {
    /// Loads libVLC (once per process), creates the probe's instance and starts its worker,
    /// which answers through `answers`.
    pub fn new(answers: LengthSender) -> Result<Self> {
        let vlc = super::library()?;
        let instance = super::new_instance(&vlc, &ARGUMENTS)?;
        let worker = Worker { vlc, instance, answers, waiting: VecDeque::new() };
        let (sender, inbox) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("libvlc-lengths".into())
            .spawn(move || worker.run(&inbox))
            .context("starting the length probe's thread")?;
        Ok(Self { worker: sender, thread: Some(thread) })
    }

    fn send(&self, message: Message) {
        // An error means the worker is gone; lengths then just stop arriving.
        let _ = self.worker.send(message);
    }
}

impl LengthProbe for LibVlcLengths {
    fn probe(&mut self, media: Vec<Media>) {
        if !media.is_empty() {
            self.send(Message::Probe(media));
        }
    }

    fn clear(&mut self) {
        self.send(Message::Clear);
    }
}

impl Drop for LibVlcLengths {
    /// Stops the worker without waiting for a parse in progress: dropping happens on the UI
    /// thread, and the worker exits once it's done.
    fn drop(&mut self) {
        self.send(Message::Shutdown);
        if let Some(thread) = self.thread.take()
            && thread.is_finished()
            && thread.join().is_err()
        {
            tracing::error!("lengths: the worker thread panicked");
        }
    }
}

/// The worker's state: its libVLC instance and what's waiting to be read.
struct Worker {
    vlc: Arc<LibVlc>,
    instance: *mut Instance,
    answers: LengthSender,
    waiting: VecDeque<Media>,
}

// SAFETY: the instance is only used on the worker's thread after it moves there.
unsafe impl Send for Worker {}

impl Worker {
    fn run(mut self, inbox: &mpsc::Receiver<Message>) {
        while self.receive(inbox) {
            if let Some(media) = self.waiting.pop_front() {
                let length = self.length(&media);
                self.answers.send((media, length));
            }
        }
    }

    /// Takes in everything that arrived, waiting only when there's nothing left to read (a
    /// Clear drops what came before it). Returns `false` when it's time to exit.
    fn receive(&mut self, inbox: &mpsc::Receiver<Message>) -> bool {
        if self.waiting.is_empty() {
            let Ok(message) = inbox.recv() else { return false };
            if !self.take(message) {
                return false;
            }
        }
        inbox.try_iter().all(|message| self.take(message))
    }

    /// Takes in `message`; `false` when it's time to exit.
    fn take(&mut self, message: Message) -> bool {
        match message {
            Message::Probe(media) => self.waiting.extend(media),
            Message::Clear => self.waiting.clear(),
            Message::Shutdown => return false,
        }
        true
    }

    /// `media`'s length: `None` for network media, and for files that can't be parsed or say
    /// they have none.
    fn length(&self, media: &Media) -> Option<Duration> {
        let path = CString::new(media.as_path()?.to_string_lossy().into_owned()).ok()?;
        let api = &self.vlc.api;
        // SAFETY: the instance is live and `path` is NUL-terminated.
        let handle = unsafe { (api.libvlc_media_new_path)(self.instance, path.as_ptr()) };
        if handle.is_null() {
            return None;
        }
        // SAFETY: `handle` is a live media of this instance, released once below.
        unsafe {
            let parsed = parse_headers(&self.vlc, handle, PARSE_TIMEOUT);
            let millis = if parsed { (api.libvlc_media_get_duration)(handle) } else { -1 };
            (api.libvlc_media_release)(handle);
            u64::try_from(millis).ok().filter(|&ms| ms > 0).map(Duration::from_millis)
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        // SAFETY: the instance is live and released once, after every media of it.
        unsafe { (self.vlc.api.libvlc_release)(self.instance) };
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use futures::StreamExt;

    use super::*;
    use crate::engine::lengths::channel;

    fn media(name: &str) -> Media {
        Media::File(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/spike-media").join(name))
    }

    #[test]
    #[ignore = "needs libVLC and the S1.1 test media (see `libvlc/tests.rs`)"]
    fn reads_lengths_of_files_and_none_for_the_rest() {
        // SAFETY: the ignored libVLC tests run on one thread (`--test-threads 1`).
        unsafe { crate::engine::libvlc::runtime::prepare_environment() };
        let (sender, receiver) = channel();
        let mut probe = LibVlcLengths::new(sender).expect("libVLC loads");
        let video = media("1080p60-h264.mp4");
        let missing = media("no-such-file.mp4");
        let stream = Media::Url("https://example.invalid/live".into());
        probe.probe(vec![video.clone(), missing.clone(), stream.clone()]);
        let answers: Vec<_> = futures::executor::block_on(receiver.take(3).collect());
        assert_eq!(
            answers,
            [(video, Some(Duration::from_secs(70))), (missing, None), (stream, None)]
        );
    }
}
