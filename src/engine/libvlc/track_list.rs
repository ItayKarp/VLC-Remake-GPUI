//! Builds the engine's [`Tracks`] from what libVLC 3 reports: the media's elementary streams
//! (language, title, codec, channels, video size) and the player's track lists (which tracks
//! exist now, and the selection).
//!
//! libVLC 3 reports neither of a container's default and forced flags, so both are inferred:
//! a track is forced when its title says so ("English (Forced)"), and the default of each kind
//! is the track libVLC plays by itself once playback starts, before any command picks one:
//! libVLC's choice follows the default flag.

use std::collections::HashMap;

use super::api::{MediaTrack, borrowed_string};
use super::query::display_size;
use crate::engine::display_text::clean_tag;
use crate::engine::language::Language;
use crate::engine::media_info::VideoSize;
use crate::engine::tracks::{Track, TrackId, TrackKind, Tracks, codec};

/// The word a forced track's title carries.
const FORCED: &str = "forced";

/// One elementary stream as `libvlc_media_tracks_get` describes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Stream {
    pub(super) id: TrackId,
    pub(super) kind: Option<TrackKind>,
    pub(super) language: Option<String>,
    /// The container's track title (`psz_description`).
    pub(super) title: Option<String>,
    pub(super) codec: Option<String>,
    pub(super) channels: Option<u32>,
    pub(super) size: Option<VideoSize>,
}

/// Reads one `libvlc_media_track_t`.
///
/// # Safety
///
/// `track` is live, and its detail is NULL or live details of its kind.
pub(super) unsafe fn stream(track: &MediaTrack) -> Stream {
    let kind = match track.kind {
        0 => Some(TrackKind::Audio),
        1 => Some(TrackKind::Video),
        2 => Some(TrackKind::Subtitle),
        _ => None,
    };
    // SAFETY: the detail matches the kind, and each is NULL or live.
    let (audio, video) = unsafe {
        match kind {
            Some(TrackKind::Audio) => (track.detail.audio.as_ref(), None),
            Some(TrackKind::Video) => (None, track.detail.video.as_ref()),
            _ => (None, None),
        }
    };
    Stream {
        id: TrackId(track.id),
        kind,
        // SAFETY: libVLC's strings are NULL or NUL-terminated.
        language: unsafe { borrowed_string(track.language) },
        // SAFETY: as above.
        title: clean_tag(unsafe { borrowed_string(track.description) }),
        codec: codec::name(track.codec),
        channels: audio.map(|audio| audio.channels).filter(|channels| *channels > 0),
        size: video.and_then(display_size),
    }
}

/// The tracks of `kind`: the player's list once playing (it has tracks added later, such as
/// subtitle files), else the media's streams. Details come from the streams; the player's
/// "Disable" entry (id -1) is left out.
pub(super) fn merge(kind: TrackKind, listed: &[TrackId], streams: &[Stream]) -> Vec<Track> {
    let listed: Vec<TrackId> = listed.iter().copied().filter(|id| id.0 >= 0).collect();
    if listed.is_empty() {
        return streams
            .iter()
            .filter(|stream| stream.kind == Some(kind))
            .map(|stream| track(kind, stream.id, Some(stream)))
            .collect();
    }
    listed
        .into_iter()
        .map(|id| track(kind, id, streams.iter().find(|stream| stream.id == id)))
        .collect()
}

fn track(kind: TrackKind, id: TrackId, stream: Option<&Stream>) -> Track {
    let Some(stream) = stream else { return Track::new(id, kind) };
    Track {
        title: stream.title.clone(),
        language: stream.language.as_deref().and_then(Language::parse),
        codec: stream.codec.clone(),
        channels: stream.channels,
        forced: stream.title.as_deref().is_some_and(says_forced),
        ..Track::new(id, kind)
    }
}

fn says_forced(title: &str) -> bool {
    title.split(|c: char| !c.is_alphanumeric()).any(|word| word.eq_ignore_ascii_case(FORCED))
}

/// The track libVLC picked by itself for each kind in the open media: the container's default.
#[derive(Debug, Default)]
pub(super) struct Defaults {
    /// Per kind: the default learned, or `None` once a command picked first.
    learned: HashMap<TrackKind, Option<TrackId>>,
}

impl Defaults {
    /// Forgets the last media's defaults.
    pub(super) fn reset(&mut self) {
        self.learned.clear();
    }

    /// A command picked a track of `kind`: later selections are no longer libVLC's own.
    pub(super) fn commanded(&mut self, kind: TrackKind) {
        self.learned.entry(kind).or_insert(None);
    }

    /// Learns libVLC's own picks from `tracks`. Call it only once playback started: before,
    /// libVLC holds whichever track it met first, and switches to the flagged one later.
    pub(super) fn learn(&mut self, tracks: &Tracks) {
        for kind in [TrackKind::Audio, TrackKind::Video, TrackKind::Subtitle] {
            if let Some(id) = tracks.selected(kind) {
                self.learned.entry(kind).or_insert(Some(id));
            }
        }
    }

    /// Marks the default tracks learned so far in `tracks`.
    pub(super) fn mark(&self, tracks: &mut Tracks) {
        for track in &mut tracks.all {
            track.default = self.learned.get(&track.kind) == Some(&Some(track.id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream(id: i32, language: &str, title: Option<&str>) -> Stream {
        Stream {
            id: TrackId(id),
            kind: Some(TrackKind::Subtitle),
            language: Some(language.into()),
            title: title.map(Into::into),
            codec: Some("SRT".into()),
            channels: None,
            size: None,
        }
    }

    #[test]
    fn the_players_list_wins_and_streams_fill_in_before_playback() {
        let streams = [stream(1, "en", None), stream(2, "fr", Some("French Forced"))];
        let listed = merge(TrackKind::Subtitle, &[TrackId(-1), TrackId(2), TrackId(7)], &streams);
        assert_eq!(listed.iter().map(|t| t.id.0).collect::<Vec<_>>(), [2, 7]);
        assert_eq!(listed[0].language, Language::parse("fra"));
        assert!(listed[0].forced);
        assert_eq!(listed[1], Track::new(TrackId(7), TrackKind::Subtitle));
        let unlisted = merge(TrackKind::Subtitle, &[], &streams);
        assert_eq!(unlisted.iter().map(|t| t.id.0).collect::<Vec<_>>(), [1, 2]);
        assert!(!unlisted[0].forced);
        assert_eq!(merge(TrackKind::Audio, &[], &streams), []);
    }

    #[test]
    fn only_the_word_forced_marks_a_track() {
        assert!(says_forced("English (FORCED)"));
        assert!(!says_forced("Unforced errors"));
    }

    #[test]
    fn defaults_are_libvlcs_own_first_picks() {
        let mut defaults = Defaults::default();
        let mut tracks = Tracks {
            all: vec![
                Track::new(TrackId(1), TrackKind::Audio),
                Track::new(TrackId(2), TrackKind::Audio),
                Track::new(TrackId(3), TrackKind::Subtitle),
            ],
            audio: Some(TrackId(2)),
            ..Tracks::default()
        };
        defaults.commanded(TrackKind::Subtitle);
        defaults.learn(&tracks);
        tracks.audio = Some(TrackId(1));
        tracks.subtitle = Some(TrackId(3));
        defaults.learn(&tracks);
        defaults.mark(&mut tracks);
        let flagged: Vec<_> = tracks.all.iter().filter(|t| t.default).map(|t| t.id.0).collect();
        assert_eq!(flagged, [2]);
    }
}
