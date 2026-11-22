//! Subtitle files the app draws itself (S6.2). When the selected subtitle track is a file whose
//! cues were read ([`crate::engine::cues`]), the backend is told to show no subtitle and the
//! player keeps the track selected, so the video view draws its cues in the user's style.
//! Styled ASS/SSA files go to the backend instead while Embedded Sub Format is on, so their own
//! look shows.
//!
//! libVLC may select a file's track itself (one the user added shows at once): the player takes
//! that over on the next track list, and turns the backend's selection off again.

use std::time::Instant;

use gpui::Context;

use super::{Player, PlayerEvent};
use crate::engine::cues::{Cue, CueTrack};
use crate::engine::state::PlaybackState;
use crate::engine::tracks::{TrackId, TrackKind};

impl Player {
    /// Embedded Sub Format: on, styled (ASS/SSA) files keep their own look, drawn by the
    /// backend; off, the app draws them in the user's style. Applies to the subtitle showing at
    /// once.
    pub fn set_file_styles(&mut self, on: bool, cx: &mut Context<Self>) {
        if self.file_styles == on {
            return;
        }
        self.file_styles = on;
        if let Some(track) = self.info.tracks.subtitle {
            self.apply_subtitle(Some(track));
            cx.emit(PlayerEvent::TracksChanged);
            cx.notify();
        }
    }

    /// The cues the app draws now, the subtitle sync applied (S6.3); `None` while the backend
    /// draws the subtitle or none shows.
    pub fn drawn_subtitles(&self, now: Instant) -> Option<Vec<&Cue>> {
        let cues = self.drawable(self.info.drawn?)?;
        let running = self.state == PlaybackState::Playing;
        let time = self.position.at(now, self.controls.rate.get(), running);
        let sync = self.subtitle_sync();
        Some(cues.at_stretched(sync.file_time(time), sync.duration_factor()))
    }

    /// Whether the subtitle showing is drawn by the backend into the picture: a picture format
    /// or a styled file, which the appearance settings don't reach.
    pub fn backend_draws_subtitle(&self) -> bool {
        self.info.tracks.subtitle.is_some() && self.info.drawn.is_none()
    }

    /// Selects subtitle `track`: drawn by the app when it can be, else by the backend.
    pub(super) fn apply_subtitle(&mut self, track: Option<TrackId>) {
        let drawn = track.filter(|id| self.drawable(*id).is_some());
        self.info.drawn = drawn;
        let backend_track = if drawn.is_some() { None } else { track };
        self.with_backend(|backend| backend.select_track(TrackKind::Subtitle, backend_track));
        self.info.tracks.select(TrackKind::Subtitle, track);
        self.push_subtitle_delay();
    }

    /// Squares the backend's new track list with what the app draws: a file track the backend
    /// selected by itself is taken over, and while the app draws one the backend shows none.
    pub(super) fn reconcile_drawn(&mut self) {
        let reported = self.info.tracks.subtitle;
        if let Some(track) = reported.filter(|id| self.drawable(*id).is_some()) {
            self.info.drawn = Some(track);
        }
        let Some(drawn) = self.info.drawn else { return };
        if reported.is_some() {
            self.with_backend(|backend| backend.select_track(TrackKind::Subtitle, None));
        }
        self.info.tracks.select(TrackKind::Subtitle, Some(drawn));
    }

    /// The cues of subtitle `track`, when the app draws it.
    fn drawable(&self, track: TrackId) -> Option<&CueTrack> {
        let file = self.info.subtitle_files.iter().find(|file| file.track == Some(track))?;
        let cues = file.cues.as_deref()?;
        (!(cues.is_styled() && self.file_styles)).then_some(cues)
    }
}
