//! libVLC's event callback (S1.2.4). It runs on libVLC's own threads, often with libVLC locks
//! held, so it never calls libVLC: it translates the event and passes it on, either straight to
//! the player or, when answering needs a libVLC query, to the backend's worker thread.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError, mpsc};
use std::time::Duration;

use super::api::Event;
use super::worker::{Message, Refresh};
use crate::engine::backend::{BackendEvent, EventSender};
use crate::engine::state::{ErrorKind, PlaybackState};
use crate::engine::values::Generation;

/// `libvlc_MediaMetaChanged`, attached on each media.
pub const MEDIA_META_CHANGED: i32 = 0;
const OPENING: i32 = 0x102;
const BUFFERING: i32 = 0x103;
const PLAYING: i32 = 0x104;
pub(super) const PAUSED: i32 = 0x105;
const STOPPED: i32 = 0x106;
pub(super) const END_REACHED: i32 = 0x109;
pub(super) const ENCOUNTERED_ERROR: i32 = 0x10A;
pub(super) const TIME_CHANGED: i32 = 0x10B;
const SEEKABLE_CHANGED: i32 = 0x10D;
const PAUSABLE_CHANGED: i32 = 0x10E;
const TITLE_CHANGED: i32 = 0x10F;
const LENGTH_CHANGED: i32 = 0x111;
const VOUT: i32 = 0x112;
const SCRAMBLED_CHANGED: i32 = 0x113;
const ES_ADDED: i32 = 0x114;
const ES_DELETED: i32 = 0x115;
const ES_SELECTED: i32 = 0x116;
const CORKED: i32 = 0x117;
const UNCORKED: i32 = 0x118;
const AUDIO_DEVICE: i32 = 0x11C;

/// The media player events the backend listens to.
pub const PLAYER_EVENTS: [i32; 20] = [
    OPENING,
    BUFFERING,
    PLAYING,
    PAUSED,
    STOPPED,
    END_REACHED,
    ENCOUNTERED_ERROR,
    TIME_CHANGED,
    SEEKABLE_CHANGED,
    PAUSABLE_CHANGED,
    TITLE_CHANGED,
    LENGTH_CHANGED,
    VOUT,
    SCRAMBLED_CHANGED,
    ES_ADDED,
    ES_DELETED,
    ES_SELECTED,
    CORKED,
    UNCORKED,
    AUDIO_DEVICE,
];

/// A full buffer, in libVLC's `new_cache` percent.
const BUFFER_FULL: f32 = 100.0;

/// What the callback needs, shared with the worker. libVLC holds a raw pointer to it, so it must
/// outlive every callback: the worker drops it only after releasing the media player.
pub struct Shared {
    /// Events for the player.
    pub events: EventSender,
    /// Messages for the worker.
    pub worker: mpsc::Sender<Message>,
    /// The generation of the open libVLC is playing. Set by the worker after `set_media`, so
    /// events from stopping the previous media keep the previous generation.
    pub generation: Mutex<Generation>,
    /// Whether this open has reached `Playing`.
    pub started: AtomicBool,
    /// Whether libVLC last reported `Paused`, to return there after buffering.
    pub paused: AtomicBool,
    /// Whether a seek waits for libVLC's next time report to complete.
    pub seeking: AtomicBool,
    /// Whether the worker is reopening the media where it is ([`super::reopen`]): its opening
    /// and stopping are hidden until it plays again.
    pub switching: AtomicBool,
    /// Whether paused media reopened for new filters plays until its first picture shows
    /// (`hold.rs`): it is reported paused meanwhile.
    pub holding: AtomicBool,
}

impl Shared {
    /// The generation events are tagged with now.
    pub fn generation(&self) -> Generation {
        *self.generation.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Sends `event` to the player, tagged with the current generation.
    pub fn send(&self, event: BackendEvent) {
        self.events.send(self.generation(), event);
    }

    fn state(&self, state: PlaybackState) {
        self.send(BackendEvent::State(state));
    }

    fn ask_worker(&self, message: Message) {
        // An error means the worker exited during shutdown; nothing is left to do.
        let _ = self.worker.send(message);
    }

    fn handle(&self, event: &Event) {
        // SAFETY (for every union read below): libVLC fills the member that matches `kind`,
        // and each read here matches it (`libvlc_events.h`).
        let switching = self.switching.load(Ordering::Relaxed);
        match event.kind {
            OPENING | STOPPED if switching => {}
            OPENING => self.state(PlaybackState::Opening),
            BUFFERING => self.buffering(unsafe { event.data.float }),
            PLAYING => self.playing(),
            PAUSED => self.paused(),
            STOPPED => self.state(PlaybackState::Stopped),
            END_REACHED => self.ask_worker(Message::Ended),
            ENCOUNTERED_ERROR => self.ask_worker(Message::Failed),
            TIME_CHANGED => self.time_changed(unsafe { event.data.time }),
            SEEKABLE_CHANGED => self.send(BackendEvent::Seekable(unsafe { event.data.int } != 0)),
            PAUSABLE_CHANGED => self.send(BackendEvent::Pausable(unsafe { event.data.int } != 0)),
            LENGTH_CHANGED => self.send(BackendEvent::Duration(millis(unsafe { event.data.time }))),
            SCRAMBLED_CHANGED if unsafe { event.data.int } != 0 => {
                self.state(PlaybackState::Error(ErrorKind::Protected));
            }
            TITLE_CHANGED => self.ask_worker(Message::Refresh(Refresh::Chapters)),
            VOUT | ES_ADDED | ES_DELETED | ES_SELECTED => {
                self.ask_worker(Message::Refresh(Refresh::Tracks));
            }
            MEDIA_META_CHANGED => self.ask_worker(Message::Refresh(Refresh::Metadata)),
            CORKED => self.send(BackendEvent::Corked(true)),
            UNCORKED => self.send(BackendEvent::Corked(false)),
            AUDIO_DEVICE => self.ask_worker(Message::Refresh(Refresh::AudioDevices)),
            _ => {}
        }
    }

    fn buffering(&self, percent: f32) {
        self.send(BackendEvent::Buffering(percent));
        if percent < BUFFER_FULL {
            self.state(PlaybackState::Buffering);
        } else if self.started.load(Ordering::Relaxed) {
            let paused = self.paused.load(Ordering::Relaxed);
            self.state(if paused { PlaybackState::Paused } else { PlaybackState::Playing });
        }
    }

    fn playing(&self) {
        let holding = self.holding.load(Ordering::Relaxed);
        if !holding {
            self.paused.store(false, Ordering::Relaxed);
        }
        let first = !self.started.swap(true, Ordering::Relaxed);
        let switched = self.switching.swap(false, Ordering::Relaxed);
        if !holding {
            self.state(PlaybackState::Playing);
        }
        // Reopened media needs its tracks restored and per-media settings applied again.
        if first || switched {
            self.ask_worker(Message::Refresh(Refresh::Everything));
        }
    }

    /// Paused, which media opened with `start-paused` (S3.4) is before it ever plays: that
    /// counts as started too, so its tracks and per-media settings are read.
    fn paused(&self) {
        self.holding.store(false, Ordering::Relaxed);
        self.paused.store(true, Ordering::Relaxed);
        let first = !self.started.swap(true, Ordering::Relaxed);
        self.state(PlaybackState::Paused);
        if first {
            self.ask_worker(Message::Refresh(Refresh::Everything));
        }
    }

    fn time_changed(&self, time: i64) {
        self.send(BackendEvent::Position(Duration::from_millis(time.max(0) as u64)));
        if self.seeking.swap(false, Ordering::Relaxed) {
            self.send(BackendEvent::SeekCompleted);
        }
    }
}

/// A length libVLC reports in milliseconds; 0 and -1 mean unknown.
pub fn millis(value: i64) -> Option<Duration> {
    (value > 0).then(|| Duration::from_millis(value as u64))
}

/// The `libvlc_callback_t` registered for every event, with a `*const Shared` as user data.
pub unsafe extern "C" fn on_event(event: *const Event, data: *mut c_void) {
    if event.is_null() || data.is_null() {
        return;
    }
    // SAFETY: libVLC passes a live event, and `data` is the `Shared` the worker keeps alive
    // until the player is released (see `Shared`).
    let (event, shared) = unsafe { (&*event, &*data.cast::<Shared>()) };
    shared.handle(event);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_lengths_are_none() {
        assert_eq!(millis(-1), None);
        assert_eq!(millis(0), None);
        assert_eq!(millis(1500), Some(Duration::from_millis(1500)));
    }
}
