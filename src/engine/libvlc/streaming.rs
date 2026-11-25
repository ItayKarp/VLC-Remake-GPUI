//! Streaming out (S8.12) on the worker thread: the media's `sout` options, added to every media
//! made while streaming ([`Worker::new_media`]), and the reopen that starts or stops a stream of
//! the media playing where it is.
//!
//! A stream that can't start (its port taken after all, an address that can't be reached) makes
//! libVLC fail the whole media before it plays. The media then reopens without it, where it was
//! to start, and the player hears that the stream failed rather than that the media did.

use std::ffi::{CString, c_int};
use std::time::Duration;

use super::api::Media;
use super::core_vars::Var;
use super::reopen::Output;
use super::worker::Worker;
use crate::engine::backend::BackendEvent;
use crate::engine::stream_out::{Local, StreamTarget};

/// `libvlc_state_t`'s Opening, Buffering, Playing and Paused: media that is going.
const GOING: std::ops::RangeInclusive<c_int> = 1..=4;

/// The worker's streaming state.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Streaming {
    /// The stream the open media goes to; `None` while it doesn't stream.
    target: Option<StreamTarget>,
    /// Where the media was when the stream started, to go on from without it if it fails.
    from: Duration,
}

impl Worker {
    /// Streams the open media as `target` says, or stops, from where it is: going media reopens
    /// there; stopped or ended media gets the options for when it plays again.
    pub(super) fn stream(&mut self, target: Option<StreamTarget>) {
        if self.streaming.target == target {
            return;
        }
        self.streaming = Streaming { target, from: self.playing_time() };
        if self.source.is_none() {
            return;
        }
        if self.is_going() {
            self.reopen(self.playing_tracks(), Output::Rebuilt);
        } else {
            self.prepare_again();
        }
    }

    /// Streams the media opening now as its start says.
    pub(super) fn stream_on_open(&mut self, target: Option<StreamTarget>, from: Duration) {
        self.streaming = Streaming { target, from };
    }

    /// Adds the stream's options to `media`, while streaming.
    pub(super) fn add_stream_options(&self, media: *mut Media) {
        let Some(target) = self.streaming.target else { return };
        let options = target.media_options();
        let options: Vec<&str> = options.iter().map(String::as_str).collect();
        self.add_option(media, &options);
        for (name, value) in target.sign_in_vars() {
            let (Ok(name), Ok(value)) = (CString::new(name), CString::new(value)) else { continue };
            // SAFETY: the player is live.
            unsafe { self.vlc.set_player_var(self.handles.player, &name, Var::String(&value)) };
        }
    }

    /// Whether the open media streams.
    pub(super) fn is_streaming(&self) -> bool {
        self.streaming.target.is_some()
    }

    /// Whether the media goes only to the stream, so nothing shows or sounds here.
    pub(super) fn streams_only(&self) -> bool {
        self.streaming.target.is_some_and(|target| target.local == Local::StreamOnly)
    }

    /// The media failed: when it was a stream that never started, reopens the media without
    /// it, says so, and returns `true`.
    pub(super) fn stream_failed(&mut self) -> bool {
        let started = self.shared.started.load(std::sync::atomic::Ordering::Relaxed);
        if self.streaming.target.is_none() || started || self.source.is_none() {
            return false;
        }
        tracing::warn!("streaming: the stream couldn't start: {}", self.vlc.last_error());
        let from = self.streaming.from;
        self.streaming = Streaming::default();
        self.reopen_from(self.playing_tracks(), Output::Rebuilt, from);
        self.shared.send(BackendEvent::StreamFailed);
        true
    }

    /// Whether the media is opening, playing or paused, rather than stopped, ended or failed.
    fn is_going(&self) -> bool {
        // SAFETY: the player is live.
        let state = unsafe { (self.vlc.api.libvlc_media_player_get_state)(self.handles.player) };
        GOING.contains(&state)
    }

    /// Where the media is now; zero before it plays.
    pub(super) fn playing_time(&self) -> Duration {
        // SAFETY: the player is live.
        let millis = unsafe { (self.vlc.api.libvlc_media_player_get_time)(self.handles.player) };
        Duration::from_millis(u64::try_from(millis).unwrap_or_default())
    }

    /// Makes the stopped media anew with the stream's options as they are now, to play from its
    /// start when asked, with its options and the files added to it.
    fn prepare_again(&mut self) {
        let Some(source) = self.source.clone() else { return };
        let media = self.new_media(&source);
        if media.is_null() {
            return;
        }
        self.add_media_options(media);
        self.add_slaves(media);
        self.replace_media(media);
    }
}
