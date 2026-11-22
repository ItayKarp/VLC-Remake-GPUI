//! The libVLC preview decoder (S2.3.3): a second libVLC instance and media player, muted,
//! without subtitles, decoding in software into small BGRA pictures, on its own worker thread.
//! The main player is never touched.
//!
//! [`flow`](crate::engine::preview::flow) decides what to do; [`worker`] does it with libVLC
//! calls, and a [`FrameSink`] catches the pictures.

#[cfg(test)]
mod tests;
mod worker;

use std::ffi::c_void;
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};

use anyhow::{Context as _, Result, bail};

use self::worker::{Message, Parts, PreviewWorker};
use super::api::Event;
use super::events::{ENCOUNTERED_ERROR, END_REACHED, PAUSED, TIME_CHANGED};
use super::frame_sink::FrameSink;
use super::vmem::{self, Vmem};
use crate::app::media::Media;
use crate::engine::preview::{self, PreviewDecoder, PreviewRequest, PreviewSender};

/// The preview instance's arguments: no audio output, subtitles, on-screen text or title, and
/// software decoding (hardware decoding drops rotated video and competes with the main player).
/// The thumbnailer's instance (S4.4.1) starts the same way.
pub(super) const ARGUMENTS: [&str; 6] = [
    "--no-video-title-show",
    "--no-audio",
    "--no-spu",
    "--no-osd",
    "--no-sub-autodetect-file",
    "--avcodec-hw=none",
];

/// A [`PreviewDecoder`] on libVLC.
pub struct LibVlcPreview {
    worker: mpsc::Sender<Message>,
    thread: Option<JoinHandle<()>>,
}

impl LibVlcPreview {
    /// Loads libVLC (once per process), creates the preview instance and player, and starts the
    /// worker thread that answers through `events`.
    pub fn new(events: PreviewSender) -> Result<Self> {
        let vlc = super::library()?;
        let instance = super::new_instance(&vlc, &ARGUMENTS)?;
        // SAFETY: the instance is live.
        let player = unsafe { (vlc.api.libvlc_media_player_new)(instance) };
        if player.is_null() {
            let error = vlc.last_error();
            // SAFETY: the instance is live and released once.
            unsafe { (vlc.api.libvlc_release)(instance) };
            bail!("libvlc_media_player_new: {error}");
        }
        let (sender, inbox) = mpsc::channel();
        let shown = sender.clone();
        let sink = Arc::new(FrameSink::new(
            (preview::MAX_WIDTH, preview::MAX_HEIGHT),
            // An error means the worker exited during shutdown.
            Box::new(move || drop(shown.send(Message::Picture))),
        ));
        let vmem = Arc::new(Vmem::new(sink.clone()));
        vmem::install(&vlc, player, &vmem);
        // Boxed so its address stays put for the event callback.
        let callback = Box::new(sender.clone());
        let parts = Parts { instance, player, sink, vmem, callback };
        let worker = PreviewWorker::new(vlc, parts, events);
        worker.attach(on_event, &[PAUSED, TIME_CHANGED, END_REACHED, ENCOUNTERED_ERROR]);
        let thread = thread::Builder::new()
            .name("libvlc-preview".into())
            .spawn(move || worker.run(&inbox))
            .context("starting the preview thread")?;
        Ok(Self { worker: sender, thread: Some(thread) })
    }

    fn send(&self, message: Message) {
        // An error means the worker is gone; previews then just stop arriving.
        let _ = self.worker.send(message);
    }
}

impl PreviewDecoder for LibVlcPreview {
    fn open(&mut self, media: &Media) {
        self.send(Message::Open(media.clone()));
    }

    fn request(&mut self, request: PreviewRequest) {
        self.send(Message::Request(request));
    }

    fn close(&mut self) {
        self.send(Message::Close);
    }
}

impl Drop for LibVlcPreview {
    /// Stops the worker. Waits for it unless it may be stuck on a hung source: dropping happens
    /// on the UI thread, so a stuck worker is left to finish on its own.
    fn drop(&mut self) {
        self.send(Message::Shutdown);
        if let Some(thread) = self.thread.take()
            && thread.is_finished()
            && thread.join().is_err()
        {
            tracing::error!("preview: the worker thread panicked");
        }
    }
}

/// libVLC's event callback for the preview player: passes pausing, time changes, the end and
/// errors to the worker.
/// It never calls libVLC.
unsafe extern "C" fn on_event(event: *const Event, data: *mut c_void) {
    // SAFETY: libVLC passes a live event, and `data` is the boxed sender the worker owns until
    // after it releases the player.
    let (kind, worker) = unsafe { ((*event).kind, &*data.cast::<mpsc::Sender<Message>>()) };
    let message = match kind {
        PAUSED => Message::Paused,
        TIME_CHANGED => Message::TimeChanged,
        END_REACHED => Message::Ended,
        ENCOUNTERED_ERROR => Message::Failed,
        _ => return,
    };
    // An error means the worker exited during shutdown.
    let _ = worker.send(message);
}
