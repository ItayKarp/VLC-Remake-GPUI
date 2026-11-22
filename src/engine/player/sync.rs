//! The [`Player`]'s subtitle synchronization (S6.3): a delay, speed and duration factor per
//! subtitle track of the open media, named by [`SubtitleKey`] so a file read again in another
//! encoding keeps its sync, and a delay set for a file doesn't follow to an embedded track.
//!
//! The backend has one subtitle delay for whatever it shows, and keeps it from one media to the
//! next, so the player gives it the selected track's delay whenever that could have changed.
//! Speed and the duration factor only reach the subtitles the app draws (S6.2); the backend has
//! no way to apply them to the ones it draws.

use gpui::Context;

use super::{Player, PlayerEvent};
use crate::app::media::Media;
use crate::engine::subtitle_sync::{SubtitleKey, SubtitleSync};
use crate::engine::tracks::TrackKind;

impl Player {
    /// The selected subtitle track's key; `None` while no subtitle shows.
    pub fn subtitle_key(&self) -> Option<SubtitleKey> {
        let track = self.info.tracks.selected(TrackKind::Subtitle)?;
        let file = self.info.subtitle_files.iter().find(|file| file.track == Some(track));
        Some(file.map_or(SubtitleKey::Stream(track.0), |file| SubtitleKey::File(file.path.clone())))
    }

    /// The selected subtitle track's sync; the default while none shows.
    pub fn subtitle_sync(&self) -> SubtitleSync {
        self.subtitle_key().map_or_default(|key| self.sync_of(&key))
    }

    /// Whether the speed and duration factor reach the subtitle showing: the app draws it.
    pub fn sync_stretches(&self) -> bool {
        self.info.drawn.is_some()
    }

    /// Sets the selected subtitle track's sync. Returns the track's key, or `None`, changing
    /// nothing, while no subtitle shows.
    pub fn set_subtitle_sync(
        &mut self,
        sync: SubtitleSync,
        cx: &mut Context<Self>,
    ) -> Option<SubtitleKey> {
        let key = self.subtitle_key()?;
        if sync.is_default() {
            self.info.syncs.remove(&key);
        } else {
            self.info.syncs.insert(key.clone(), sync);
        }
        self.push_subtitle_delay();
        cx.emit(PlayerEvent::SubtitleSyncChanged);
        cx.notify();
        Some(key)
    }

    /// Takes the syncs remembered for `media` (S6.3.4), unless another media opened since.
    /// A track whose sync was set meanwhile keeps that one.
    pub fn restore_subtitle_syncs(
        &mut self,
        media: &Media,
        saved: Vec<(SubtitleKey, SubtitleSync)>,
        cx: &mut Context<Self>,
    ) {
        if self.media.as_ref() != Some(media) {
            return;
        }
        for (key, sync) in saved {
            self.info.syncs.entry(key).or_insert(sync);
        }
        self.push_subtitle_delay();
        cx.emit(PlayerEvent::SubtitleSyncChanged);
        cx.notify();
    }

    /// The sync of the track `key` names.
    pub(super) fn sync_of(&self, key: &SubtitleKey) -> SubtitleSync {
        self.info.syncs.get(key).copied().unwrap_or_default()
    }

    /// Gives the backend the selected subtitle track's delay, when it holds another.
    pub(super) fn push_subtitle_delay(&mut self) {
        let delay = self.subtitle_sync().delay();
        if self.backend_subtitle_delay != delay {
            self.backend_subtitle_delay = delay;
            self.with_backend(|backend| backend.set_subtitle_delay(delay));
        }
    }
}
