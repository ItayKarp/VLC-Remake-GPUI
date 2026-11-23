//! The [`Player`]'s audio delay (S7.6.3): one per audio track of the open media, so a delay set
//! for a dubbed track or an added audio file doesn't follow to another (as subtitle delays do,
//! S6.3). A new media starts with none, then takes those remembered for it.
//!
//! The backend has one audio delay for whatever plays, and keeps it from one media to the next,
//! so the player gives it the selected track's delay whenever that could have changed.

use gpui::Context;

use super::Player;
use crate::app::media::Media;
use crate::engine::subtitle_sync::MAX_DELAY_MS;
use crate::engine::tracks::{TrackId, TrackKind};
use crate::engine::values::Offset;

impl Player {
    /// The selected audio track's delay against the video; none while no audio plays.
    pub fn audio_delay(&self) -> Offset {
        self.audio_track().map_or_default(|track| self.delay_of(track))
    }

    /// The selected audio track; `None` while no audio plays.
    pub fn audio_track(&self) -> Option<TrackId> {
        self.info.tracks.selected(TrackKind::Audio)
    }

    /// Delays the selected audio track by `delay`, kept within ten minutes either way. Returns
    /// `false`, changing nothing, while no audio track plays.
    pub fn set_audio_delay(&mut self, delay: Offset, cx: &mut Context<Self>) -> bool {
        let Some(track) = self.audio_track() else { return false };
        let delay = Offset::from_millis(delay.as_millis().clamp(-MAX_DELAY_MS, MAX_DELAY_MS));
        if delay == Offset::default() {
            self.info.audio_delays.remove(&track);
        } else {
            self.info.audio_delays.insert(track, delay);
        }
        self.push_audio_delay();
        cx.notify();
        true
    }

    /// Takes the delays remembered for `media` (S7.6.3), unless another media opened since. A
    /// track whose delay was set meanwhile keeps that one.
    pub fn restore_audio_delays(
        &mut self,
        media: &Media,
        saved: Vec<(TrackId, Offset)>,
        cx: &mut Context<Self>,
    ) {
        if self.media.as_ref() != Some(media) {
            return;
        }
        for (track, delay) in saved {
            self.info.audio_delays.entry(track).or_insert(delay);
        }
        self.push_audio_delay();
        cx.notify();
    }

    /// The delay of audio `track`.
    fn delay_of(&self, track: TrackId) -> Offset {
        self.info.audio_delays.get(&track).copied().unwrap_or_default()
    }

    /// Gives the backend the selected audio track's delay, when it holds another.
    pub(super) fn push_audio_delay(&mut self) {
        let delay = self.audio_delay();
        if self.controls.backend_audio_delay != delay {
            self.controls.backend_audio_delay = delay;
            self.with_backend(|backend| backend.set_audio_delay(delay));
        }
    }
}
