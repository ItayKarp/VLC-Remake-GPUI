//! The [`Player`]'s tracks (S1.6): selecting and cycling them, and picking them for new media
//! by the preferred languages until the user picks one of a kind.

use gpui::Context;

use super::{Player, PlayerEvent};
use crate::engine::media_options::MediaOptions;
use crate::engine::tracks::auto::{self, Pick, TrackPreferences};
use crate::engine::tracks::{Cycled, TrackId, TrackKind, Tracks, claim_added};

/// The kinds auto-selection picks, audio first: forced subtitles follow the audio's language.
const AUTO_KINDS: [TrackKind; 2] = [TrackKind::Audio, TrackKind::Subtitle];

/// What decides the open media's tracks besides the backend.
#[derive(Clone, Debug, Default)]
pub(super) struct TrackChoices {
    preferences: TrackPreferences,
    /// The kinds the user picked a track of in the open media; auto-selection leaves them be.
    picked: Vec<TrackKind>,
}

impl TrackChoices {
    /// Forgets the user's picks: new media opened.
    pub(super) fn forget_media(&mut self) {
        self.picked.clear();
    }

    /// Records that the user picked a track of `kind`.
    pub(super) fn user_picked(&mut self, kind: TrackKind) {
        if !self.picked.contains(&kind) {
            self.picked.push(kind);
        }
    }
}

impl Player {
    /// Sets the languages tracks are picked by. Tracks the user hasn't picked follow them from
    /// the next track change on.
    pub fn set_track_preferences(&mut self, preferences: TrackPreferences) {
        self.track_choices.preferences = preferences;
    }

    /// Sets the options media opens with from the next open on (S5.7).
    pub fn set_media_options(&mut self, options: MediaOptions) {
        if self.media_options != options {
            self.with_backend(|backend| backend.set_media_options(&options));
            self.media_options = options;
        }
    }

    /// Selects `track` for `kind`, or turns that kind off with `None`. Auto-selection leaves
    /// the kind alone for the rest of this media. [`Self::tracks`] shows the selection at once;
    /// the backend confirms it later.
    pub fn select_track(
        &mut self,
        kind: TrackKind,
        track: Option<TrackId>,
        cx: &mut Context<Self>,
    ) {
        self.track_choices.user_picked(kind);
        self.apply_track(kind, track);
        cx.emit(PlayerEvent::TracksChanged);
        cx.notify();
    }

    /// Selects the next track of `kind` (the B and V keys; subtitles pass through off) and
    /// says where it landed.
    pub fn cycle_track(&mut self, kind: TrackKind, cx: &mut Context<Self>) -> Cycled {
        let cycled = self.info.tracks.cycled(kind);
        match cycled {
            Cycled::NoTracks => {}
            Cycled::Off => self.select_track(kind, None, cx),
            Cycled::Track(id) => self.select_track(kind, Some(id), cx),
        }
        cycled
    }

    /// Takes the backend's track list, then picks tracks by the preferences. Runs on every
    /// change, so tracks that appear late (MPEG-TS, HLS) are picked too.
    pub(super) fn tracks_reported(&mut self, tracks: Tracks, cx: &mut Context<Self>) {
        let before = std::mem::replace(&mut self.info.tracks, tracks);
        claim_added(&mut self.info.subtitle_files, &before, &self.info.tracks);
        for kind in AUTO_KINDS {
            match self.auto_pick(kind) {
                Pick::Keep => {}
                Pick::Off => self.apply_track(kind, None),
                Pick::Track(id) => self.apply_track(kind, Some(id)),
            }
        }
        cx.emit(PlayerEvent::TracksChanged);
    }

    /// The selection the preferences ask for, when the user hasn't picked `kind` and it
    /// differs from the current one.
    fn auto_pick(&self, kind: TrackKind) -> Pick {
        if self.track_choices.picked.contains(&kind) {
            return Pick::Keep;
        }
        let (tracks, preferences) = (&self.info.tracks, &self.track_choices.preferences);
        let pick = match kind {
            TrackKind::Subtitle => auto::subtitle(tracks, preferences),
            _ => auto::audio(tracks, preferences),
        };
        pick.against(kind, tracks)
    }

    /// Asks the backend for the selection and records it now, so cycling again before the
    /// backend confirms moves on from it.
    fn apply_track(&mut self, kind: TrackKind, track: Option<TrackId>) {
        self.with_backend(|backend| backend.select_track(kind, track));
        self.info.tracks.select(kind, track);
    }
}
