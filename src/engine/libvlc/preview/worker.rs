//! The preview decoder's worker thread: the only thread that calls the preview player's libVLC
//! functions after setup. It carries out what [`Flow`] decides.

use std::ffi::{CString, c_void};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::Instant;

use crate::app::media::Media as Source;
use crate::engine::libvlc::api::{Callback, Instance, LibVlc, MediaPlayer};
use crate::engine::libvlc::decoding::SOFTWARE_DECODING;
use crate::engine::libvlc::frame_sink::FrameSink;
use crate::engine::libvlc::vmem::Vmem;
use crate::engine::preview::flow::{Action, Flow};
use crate::engine::preview::{PreviewEvent, PreviewRequest, PreviewSender};

/// Work for the preview worker.
#[derive(Debug)]
pub enum Message {
    /// Open this media for previews.
    Open(Source),
    /// Make a picture.
    Request(PreviewRequest),
    /// Close the media.
    Close,
    /// The sink caught a picture.
    Picture,
    /// The preview player paused.
    Paused,
    /// The preview player's time changed.
    TimeChanged,
    /// The preview player reached the end of the media.
    Ended,
    /// The preview player failed.
    Failed,
    /// Release everything and exit.
    Shutdown,
}

/// What the worker takes over: the libVLC handles and what their callbacks point at.
pub struct Parts {
    /// The preview instance.
    pub instance: *mut Instance,
    /// Its media player, with `vmem` installed.
    pub player: *mut MediaPlayer,
    /// Where the player's pictures land.
    pub sink: Arc<FrameSink>,
    /// The video callbacks' data.
    pub vmem: Arc<Vmem>,
    /// The event callback's data, boxed so its address stays put.
    pub callback: Box<Sender<Message>>,
}

/// The preview worker's state and libVLC handles.
pub struct PreviewWorker {
    vlc: Arc<LibVlc>,
    instance: *mut Instance,
    player: *mut MediaPlayer,
    sink: Arc<FrameSink>,
    /// Kept alive until the player is released: libVLC's video callbacks point at it.
    vmem: Option<Arc<Vmem>>,
    /// Kept alive until the player is released: the event callback points at it.
    callback: Option<Box<Sender<Message>>>,
    events: PreviewSender,
    flow: Flow,
}

// SAFETY: libVLC handles are thread-safe objects; the worker moves to its thread whole, and only
// that thread uses them afterwards.
unsafe impl Send for PreviewWorker {}

impl PreviewWorker {
    /// A worker owning `parts`, answering through `events`.
    pub fn new(vlc: Arc<LibVlc>, parts: Parts, events: PreviewSender) -> Self {
        let Parts { instance, player, sink, vmem, callback } = parts;
        let (vmem, callback) = (Some(vmem), Some(callback));
        Self { vlc, instance, player, sink, vmem, callback, events, flow: Flow::default() }
    }

    /// Routes the player's `kinds` events to `callback`, with the boxed sender as its data.
    pub fn attach(&self, callback: Callback, kinds: &[i32]) {
        let Some(sender) = &self.callback else { return };
        let data: *mut c_void = std::ptr::from_ref::<Sender<Message>>(sender).cast_mut().cast();
        // SAFETY: the player is live; its event manager lives as long as it does, and `data`
        // outlives the player (released first in `release`).
        unsafe {
            let manager = (self.vlc.api.libvlc_media_player_event_manager)(self.player);
            for &kind in kinds {
                (self.vlc.api.libvlc_event_attach)(manager, kind, callback, data);
            }
        }
    }

    /// Handles messages until [`Message::Shutdown`] or until every sender is gone.
    pub fn run(mut self, inbox: &Receiver<Message>) {
        loop {
            let received = match self.flow.deadline() {
                Some(deadline) => {
                    inbox.recv_timeout(deadline.saturating_duration_since(Instant::now()))
                }
                None => inbox.recv().map_err(|_| RecvTimeoutError::Disconnected),
            };
            let now = Instant::now();
            let actions = match received {
                Err(RecvTimeoutError::Timeout) => self.flow.tick(now),
                Err(RecvTimeoutError::Disconnected) | Ok(Message::Shutdown) => break,
                Ok(message) => self.handle(message, now),
            };
            self.apply(&actions);
        }
        self.release();
    }

    fn handle(&mut self, message: Message, now: Instant) -> Vec<Action> {
        match message {
            Message::Open(source) => {
                self.open(&source);
                self.flow.opened(now);
                Vec::new()
            }
            Message::Request(request) => self.flow.request(request, now),
            Message::Close => {
                self.close();
                self.flow.closed();
                Vec::new()
            }
            Message::Picture => self.flow.picture(now),
            Message::Paused => self.flow.paused(now),
            Message::TimeChanged => {
                self.flow.time_changed();
                Vec::new()
            }
            Message::Ended => self.flow.ended(now),
            Message::Failed => self.flow.failed(),
            Message::Shutdown => Vec::new(),
        }
    }

    fn apply(&self, actions: &[Action]) {
        let api = &self.vlc.api;
        for &action in actions {
            // SAFETY (each call): the player is live for the worker's whole life.
            unsafe {
                match action {
                    Action::Seek(time) => {
                        let millis = i64::try_from(time.as_millis()).unwrap_or(i64::MAX);
                        (api.libvlc_media_player_set_time)(self.player, millis);
                    }
                    Action::Resume => (api.libvlc_media_player_set_pause)(self.player, 0),
                    Action::Pause => (api.libvlc_media_player_set_pause)(self.player, 1),
                    Action::Deliver(id) => {
                        self.events.send(PreviewEvent { id, image: self.sink.take() });
                    }
                    Action::Fail(id) => self.events.send(PreviewEvent { id, image: None }),
                    Action::Restart => {
                        (api.libvlc_media_player_stop)(self.player);
                        (api.libvlc_media_player_play)(self.player);
                    }
                }
            }
        }
    }

    /// Plays `source` muted from the start; the flow pauses it on its first picture.
    fn open(&self, source: &Source) {
        let (text, from_path) = match source {
            Source::File(path) => (path.to_string_lossy().into_owned(), true),
            Source::Url(url) => (url.clone(), false),
        };
        let Ok(text) = CString::new(text) else { return };
        let api = &self.vlc.api;
        let create =
            if from_path { api.libvlc_media_new_path } else { api.libvlc_media_new_location };
        // SAFETY: the instance and player are live, every string is NUL-terminated, and the
        // player takes its own reference to the media before ours is released.
        unsafe {
            let media = create(self.instance, text.as_ptr());
            if media.is_null() {
                tracing::warn!("preview: couldn't create the media: {}", self.vlc.last_error());
                return;
            }
            for option in SOFTWARE_DECODING {
                let option = CString::new(option).expect("no NUL in constants");
                (api.libvlc_media_add_option)(media, option.as_ptr());
            }
            (api.libvlc_media_player_set_media)(self.player, media);
            (api.libvlc_media_release)(media);
            (api.libvlc_media_player_play)(self.player);
        }
    }

    fn close(&self) {
        // SAFETY: the player is live; NULL media is allowed.
        unsafe {
            (self.vlc.api.libvlc_media_player_stop)(self.player);
            (self.vlc.api.libvlc_media_player_set_media)(self.player, std::ptr::null_mut());
        }
    }

    fn release(mut self) {
        let api = &self.vlc.api;
        // SAFETY: the handles are live and released once. `stop` joins libVLC's threads, and
        // after the player is released no callback runs, so their data may then be dropped.
        unsafe {
            (api.libvlc_media_player_stop)(self.player);
            (api.libvlc_media_player_release)(self.player);
            (api.libvlc_release)(self.instance);
        }
        drop(self.vmem.take());
        drop(self.callback.take());
    }
}
