//! HDR on the worker thread (S7.5): reading the video's colors once it plays, and tone-mapping
//! it as the HDR mode says, reopening it unseen where that changes how its pictures arrive.
//!
//! The colors are known only a moment after the video starts (`stream_colour.rs`), so once video
//! plays the worker reads them every [`COLOUR_POLL`] for at most [`COLOUR_WATCH`]; video that
//! names no transfer function by then counts as SDR. Tone mapping needs libVLC's 10-bit
//! pictures, which it gives only when decoding in software, so HDR video opened with hardware
//! decoding is reopened in software where it is, and its first moments show washed out. A mode
//! that forces a transfer function maps from the open, in software from the start.

use std::time::{Duration, Instant};

use super::decoding::Decoding;
use super::stream_colour::ColourLines;
use super::worker::Worker;
use crate::engine::backend::BackendEvent;
use crate::engine::hdr::{HdrMode, HdrStatus, SourceColour};

/// How often the colors are read while unknown.
const COLOUR_POLL: Duration = Duration::from_millis(50);
/// How long after video starts the colors are read before it counts as SDR.
const COLOUR_WATCH: Duration = Duration::from_secs(2);

/// What the worker knows and does about HDR.
#[derive(Debug, Default)]
pub(super) struct HdrState {
    mode: HdrMode,
    /// The open media's colors, once read.
    colour: Option<SourceColour>,
    /// While the colors are read: when next, and until when.
    watch: Option<(Instant, Instant)>,
    /// What the player was told last.
    sent: Option<HdrStatus>,
}

impl Worker {
    /// Shows HDR as `mode` says from now on, the video playing included.
    pub(super) fn set_hdr_mode(&mut self, mode: HdrMode) {
        self.hdr.mode = mode;
        self.apply_hdr();
    }

    /// Forgets the colors of the media before: the next one maps only what the mode forces until
    /// its own are read. Runs before the open decides how to decode.
    pub(super) fn reset_hdr(&mut self) {
        self.hdr.colour = None;
        self.hdr.watch = None;
        self.hdr.sent = None;
        if let Some(video) = &self.video {
            video.set_tone_map(self.hdr.mode.tone_map(SourceColour::default()));
        }
    }

    /// Starts reading the colors of the video that started playing, unless they are known.
    pub(super) fn watch_colour(&mut self, now: Instant) {
        if self.hdr.colour.is_none() && self.hdr.watch.is_none() && self.shows_video() {
            self.hdr.watch = Some((now, now + COLOUR_WATCH));
        }
    }

    /// How long until the colors are read again, while they are watched.
    pub(super) fn until_colour_read(&self, now: Instant) -> Option<Duration> {
        self.hdr.watch.map(|(next, _)| next.saturating_duration_since(now))
    }

    /// Reads the colors if due; once they are known, or the watch is over, maps as they say.
    pub(super) fn read_colour_if_due(&mut self, now: Instant) {
        let Some((next, until)) = self.hdr.watch else { return };
        if now < next {
            return;
        }
        // SAFETY: the player is live.
        let lines = unsafe { self.vlc.video_colour(self.handles.player) };
        if !lines.as_ref().is_some_and(ColourLines::knows_transfer) && now < until {
            self.hdr.watch = Some((now + COLOUR_POLL, until));
            return;
        }
        self.hdr.watch = None;
        let colour = lines.map_or_default(|lines| lines.colour());
        tracing::info!("libvlc: the video's colors: {colour:?}");
        self.hdr.colour = Some(colour);
        self.apply_hdr();
    }

    /// Whether pictures are tone-mapped from the next video output, which needs software
    /// decoding.
    pub(super) fn tone_maps(&self) -> bool {
        self.video.as_ref().is_some_and(|video| video.wanted_tone_map().is_some())
    }

    /// Tone-maps as the mode and the colors say, reopening the video playing where that changes
    /// how its pictures arrive, and tells the player.
    fn apply_hdr(&mut self) {
        let Some(video) = self.video.clone() else { return };
        let map = self.hdr.mode.tone_map(self.hdr.colour.unwrap_or_default());
        let new_output = video.set_tone_map(map);
        let to_software = map.is_some() && self.decoding == Decoding::Hardware;
        if (new_output || to_software) && self.shows_video() {
            tracing::info!("libvlc: tone mapping {map:?}; reopening the video where it is");
            self.reopen_for_filters();
        }
        let status = HdrStatus { source: self.hdr.colour, tone_map: map };
        if self.hdr.sent.replace(status) != Some(status) {
            self.shared.send(BackendEvent::Hdr(status));
        }
    }
}
