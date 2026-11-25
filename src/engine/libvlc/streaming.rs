//! Streaming out (S8.12) on the worker thread: the media's `sout` options, added to every media
//! made while streaming ([`Worker::new_media`]), and the reopen that starts or stops a stream of
//! the media playing where it is.
//!
//! A stream that can't start (its port taken after all, an address that can't be reached) makes
//! libVLC fail the whole media before it plays. The media then reopens without it, where it was
//! to start, and the player hears that the stream failed rather than that the media did.
//!
//! [`state`] decides what each change asks of the media; the worker here only reads the player
//! and carries it out.

mod state;

use std::time::Duration;

use self::state::{MediaState, Retarget};
use super::api::Media;
use super::reopen::Output;
use super::worker::Worker;
use crate::engine::backend::BackendEvent;
use crate::engine::stream_out::StreamTarget;

pub(super) use self::state::Streaming;

impl Worker {
    /// Streams the open media as `target` says, or stops, from where it is: going media reopens
    /// there; stopped or ended media gets the options for when it plays again.
    pub(super) fn stream(&mut self, target: Option<StreamTarget>) {
        let (at, media) = (self.playing_time(), self.media_state());
        match self.streaming.retarget(target, at, media) {
            Retarget::Nothing => {}
            Retarget::Reopen => self.reopen(self.playing_tracks(), Output::Rebuilt),
            Retarget::Prepare => self.prepare_again(),
        }
    }

    /// Streams the media opening now as its start says.
    pub(super) fn stream_on_open(&mut self, target: Option<StreamTarget>, from: Duration) {
        self.streaming.open(target, from);
    }

    /// Adds the stream's options to `media`, while streaming.
    pub(super) fn add_stream_options(&self, media: *mut Media) {
        let Some(options) = self.streaming.media_options() else { return };
        let options: Vec<&str> = options.iter().map(String::as_str).collect();
        self.add_option(media, &options);
    }

    /// Whether the open media streams.
    pub(super) fn is_streaming(&self) -> bool {
        self.streaming.is_streaming()
    }

    /// Whether the media goes only to the stream, so nothing shows or sounds here.
    pub(super) fn streams_only(&self) -> bool {
        self.streaming.streams_only()
    }

    /// The media failed: when it was a stream that never started, reopens the media without
    /// it, says so, and returns `true`.
    pub(super) fn stream_failed(&mut self) -> bool {
        let started = self.shared.started.load(std::sync::atomic::Ordering::Relaxed);
        let Some(from) = self.streaming.fail(started, self.media_state()) else { return false };
        tracing::warn!("streaming: the stream couldn't start: {}", self.vlc.last_error());
        self.reopen_from(self.playing_tracks(), Output::Rebuilt, from);
        self.shared.send(BackendEvent::StreamFailed);
        true
    }

    /// Where the open media stands, read from the player.
    fn media_state(&self) -> MediaState {
        // SAFETY: the player is live.
        let state = unsafe { (self.vlc.api.libvlc_media_player_get_state)(self.handles.player) };
        MediaState::of(self.source.is_some(), state)
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
