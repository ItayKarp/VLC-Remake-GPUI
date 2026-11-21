//! Picking tracks for new media by the user's preferred languages (S1.6.3).
//!
//! - Audio: the first preferred language that has a track wins; among its tracks the container's
//!   default, else the first. Without a match the backend's own choice stays.
//! - Subtitles, when shown: a full (not forced) track in the first preferred language that has
//!   one; else whatever the backend chose (the container's default). With nothing shown, a
//!   forced track in the audio's language, which carries the signs and foreign lines a viewer of
//!   that audio needs.
//! - Subtitles set to forced only: the forced track in the audio's language, or off.

use super::{Track, TrackId, TrackKind, Tracks};
use crate::engine::language::Language;

/// Whether new media shows subtitles ("Enable subtitles" in Settings → Subtitles).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SubtitleMode {
    /// Show the preferred language's subtitles, or the container's default.
    #[default]
    Show,
    /// Show only forced subtitles in the audio's language.
    ForcedOnly,
}

/// What the user prefers for new media (Stage 7 settings).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrackPreferences {
    /// Audio languages, most preferred first; empty keeps the backend's choice.
    pub audio_languages: Vec<Language>,
    /// Subtitle languages, most preferred first.
    pub subtitle_languages: Vec<Language>,
    /// Whether subtitles show.
    pub subtitles: SubtitleMode,
}

/// What auto-selection does with one kind of track.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    /// Leave the selection alone.
    Keep,
    /// Turn the kind off.
    Off,
    /// Play this track.
    Track(TrackId),
}

impl Pick {
    /// This pick, or [`Self::Keep`] when it would change nothing in `tracks`.
    pub fn against(self, kind: TrackKind, tracks: &Tracks) -> Self {
        let wanted = match self {
            Self::Keep => return self,
            Self::Off => None,
            Self::Track(id) => Some(id),
        };
        if tracks.selected(kind) == wanted { Self::Keep } else { self }
    }
}

/// The audio track `preferences` ask for.
pub fn audio(tracks: &Tracks, preferences: &TrackPreferences) -> Pick {
    preferred(tracks, TrackKind::Audio, &preferences.audio_languages, |_| true)
        .map_or(Pick::Keep, Pick::Track)
}

/// The subtitle track `preferences` ask for, given the audio already selected in `tracks`.
pub fn subtitle(tracks: &Tracks, preferences: &TrackPreferences) -> Pick {
    let shown = preferences.subtitles == SubtitleMode::Show;
    if shown {
        let full = |track: &Track| !track.forced;
        if let Some(id) =
            preferred(tracks, TrackKind::Subtitle, &preferences.subtitle_languages, full)
        {
            return Pick::Track(id);
        }
        if tracks.subtitle.is_some() {
            return Pick::Keep;
        }
    }
    let audio_language = tracks.selected_track(TrackKind::Audio).and_then(|t| t.language.clone());
    let forced = audio_language
        .and_then(|language| preferred(tracks, TrackKind::Subtitle, &[language], |t| t.forced));
    match (forced, shown) {
        (Some(id), _) => Pick::Track(id),
        (None, true) => Pick::Keep,
        (None, false) => Pick::Off,
    }
}

/// The best track of `kind` in the first of `languages` that has one passing `accept`: the
/// container's default among them, else the first.
fn preferred(
    tracks: &Tracks,
    kind: TrackKind,
    languages: &[Language],
    accept: impl Fn(&Track) -> bool,
) -> Option<TrackId> {
    languages.iter().find_map(|language| {
        let matching: Vec<&Track> = tracks
            .of(kind)
            .filter(|track| track.language.as_ref() == Some(language) && accept(track))
            .collect();
        matching.iter().find(|track| track.default).or(matching.first()).map(|track| track.id)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: i32, kind: TrackKind, language: &str) -> Track {
        Track { language: Language::parse(language), ..Track::new(TrackId(id), kind) }
    }

    fn forced(id: i32, language: &str) -> Track {
        Track { forced: true, ..track(id, TrackKind::Subtitle, language) }
    }

    /// Japanese and English audio (English playing), and English, forced English and French
    /// subtitles.
    fn movie() -> Tracks {
        Tracks {
            all: vec![
                track(1, TrackKind::Audio, "jpn"),
                track(2, TrackKind::Audio, "eng"),
                track(3, TrackKind::Subtitle, "eng"),
                forced(4, "eng"),
                track(5, TrackKind::Subtitle, "fre"),
            ],
            audio: Some(TrackId(2)),
            ..Tracks::default()
        }
    }

    fn preferences(audio: &[&str], subtitles: &[&str], mode: SubtitleMode) -> TrackPreferences {
        let parse = |tags: &[&str]| tags.iter().filter_map(|tag| Language::parse(tag)).collect();
        TrackPreferences {
            audio_languages: parse(audio),
            subtitle_languages: parse(subtitles),
            subtitles: mode,
        }
    }

    #[test]
    fn audio_follows_the_first_language_that_has_a_track() {
        let movie = movie();
        let wanted = preferences(&["ko", "ja"], &[], SubtitleMode::Show);
        assert_eq!(audio(&movie, &wanted), Pick::Track(TrackId(1)));
        assert_eq!(audio(&movie, &preferences(&["ko"], &[], SubtitleMode::Show)), Pick::Keep);
    }

    #[test]
    fn the_containers_default_wins_within_a_language() {
        let mut movie = movie();
        movie.all.push(Track { default: true, ..track(6, TrackKind::Audio, "jpn") });
        let wanted = preferences(&["ja"], &[], SubtitleMode::Show);
        assert_eq!(audio(&movie, &wanted), Pick::Track(TrackId(6)));
    }

    #[test]
    fn preferred_subtitles_are_full_tracks() {
        let wanted = preferences(&[], &["fr", "en"], SubtitleMode::Show);
        assert_eq!(subtitle(&movie(), &wanted), Pick::Track(TrackId(5)));
        let english = preferences(&[], &["en"], SubtitleMode::Show);
        assert_eq!(subtitle(&movie(), &english), Pick::Track(TrackId(3)));
    }

    #[test]
    fn forced_subtitles_follow_the_audio_language() {
        let none = preferences(&[], &[], SubtitleMode::Show);
        assert_eq!(subtitle(&movie(), &none), Pick::Track(TrackId(4)));
        let forced_only = preferences(&[], &["fr"], SubtitleMode::ForcedOnly);
        assert_eq!(subtitle(&movie(), &forced_only), Pick::Track(TrackId(4)));
        let mut japanese = movie();
        japanese.audio = Some(TrackId(1));
        assert_eq!(subtitle(&japanese, &forced_only), Pick::Off);
        assert_eq!(subtitle(&japanese, &none), Pick::Keep);
    }

    #[test]
    fn a_shown_default_subtitle_is_kept() {
        let mut movie = movie();
        movie.subtitle = Some(TrackId(5));
        let none = preferences(&[], &[], SubtitleMode::Show);
        assert_eq!(subtitle(&movie, &none), Pick::Keep);
        assert_eq!(Pick::Track(TrackId(5)).against(TrackKind::Subtitle, &movie), Pick::Keep);
        assert_eq!(Pick::Off.against(TrackKind::Subtitle, &movie), Pick::Off);
    }
}
