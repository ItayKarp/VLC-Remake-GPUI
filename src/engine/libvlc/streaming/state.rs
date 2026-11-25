//! The worker's streaming state (S8.12), kept apart from libVLC: which stream the open media
//! goes to, and what starting, stopping or failing one asks of the media. The worker reads
//! the player and carries out the answer ([`super`]).

use std::ffi::c_int;
use std::time::Duration;

use crate::engine::stream_out::{Local, StreamTarget};

/// `libvlc_state_t`'s Opening, Buffering, Playing and Paused: media that is going.
const GOING: std::ops::RangeInclusive<c_int> = 1..=4;

/// Where the open media stands, as far as a change of stream cares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MediaState {
    /// No media is open.
    Closed,
    /// It is opening, playing or paused.
    Going,
    /// It is stopped, ended or failed.
    Stopped,
}

impl MediaState {
    /// The state of media that is `open` or not, whose player reports `libvlc_state_t` `state`.
    pub(crate) fn of(open: bool, state: c_int) -> Self {
        if !open {
            Self::Closed
        } else if GOING.contains(&state) {
            Self::Going
        } else {
            Self::Stopped
        }
    }
}

/// What the open media does for a change of stream to take effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Retarget {
    /// Nothing: the stream is the one asked for, or there is no media to stream.
    Nothing,
    /// Going media reopens where it is, so the new stream starts there.
    Reopen,
    /// Stopped media is made anew with the new options, for when it plays again.
    Prepare,
}

/// The stream the open media goes to, and where it started.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Streaming {
    /// The stream the open media goes to; `None` while it doesn't stream.
    target: Option<StreamTarget>,
    /// Where the media was when the stream started, to go on from without it if it fails.
    from: Duration,
}

impl Streaming {
    /// Streams the open media, which is `at` and stands as `media`, as `target` says, or
    /// stops; says what the media must do for it.
    pub(crate) fn retarget(
        &mut self,
        target: Option<StreamTarget>,
        at: Duration,
        media: MediaState,
    ) -> Retarget {
        if self.target == target {
            return Retarget::Nothing;
        }
        *self = Self { target, from: at };
        match media {
            MediaState::Closed => Retarget::Nothing,
            MediaState::Going => Retarget::Reopen,
            MediaState::Stopped => Retarget::Prepare,
        }
    }

    /// Streams the media opening now as `target` says, from `from`.
    pub(crate) fn open(&mut self, target: Option<StreamTarget>, from: Duration) {
        *self = Self { target, from };
    }

    /// The media options that stream new media; `None` while not streaming.
    pub(crate) fn media_options(&self) -> Option<Vec<String>> {
        self.target.map(|target| target.media_options())
    }

    /// Whether the open media streams.
    pub(crate) fn is_streaming(&self) -> bool {
        self.target.is_some()
    }

    /// Whether the media goes only to the stream, so nothing shows or sounds here.
    pub(crate) fn streams_only(&self) -> bool {
        self.target.is_some_and(|target| target.local == Local::StreamOnly)
    }

    /// The open media failed after it `started` or before: when it was a stream that never
    /// started, stops streaming and returns where the media goes on from without it.
    pub(crate) fn fail(&mut self, started: bool, media: MediaState) -> Option<Duration> {
        if self.target.is_none() || started || media == MediaState::Closed {
            return None;
        }
        let from = self.from;
        *self = Self::default();
        Some(from)
    }
}

#[cfg(test)]
mod tests;
