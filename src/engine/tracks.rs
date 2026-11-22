//! The track model (S1.6): the media's audio, video and subtitle tracks, which ones play, and
//! what to call them.
//!
//! - [`codec`]: short codec names ("AC3") from libVLC's FourCCs.
//! - [`label`]: display names such as "English — AC3 5.1", for tracks the container didn't
//!   title.
//! - [`auto`]: picking tracks by the preferred languages, with forced subtitles for the audio's
//!   language.

pub mod auto;
pub mod codec;
mod label;

use std::path::PathBuf;

use encoding_rs::Encoding;

use super::language::Language;
use super::sidecar::Sidecar;

/// The kind of elementary stream a track carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TrackKind {
    /// Sound.
    Audio,
    /// Pictures.
    Video,
    /// Subtitles or captions.
    Subtitle,
}

/// The backend's id for a track, unique within one media and stable while it plays, so a
/// choice survives tracks being added or removed around it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TrackId(pub i32);

/// One track of the media.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Track {
    /// The backend's id, passed back to select it.
    pub id: TrackId,
    /// What it carries.
    pub kind: TrackKind,
    /// The title the container gives it ("Director's commentary"), if any.
    pub title: Option<String>,
    /// Its language; `None` when untagged or "undetermined".
    pub language: Option<Language>,
    /// Its codec's short name ("AC3", "SRT"), if known.
    pub codec: Option<String>,
    /// An audio track's channel count.
    pub channels: Option<u32>,
    /// The container marks it as the one to play by default.
    pub default: bool,
    /// A forced subtitle: only the lines in another language than the audio (signs, foreign
    /// dialogue).
    pub forced: bool,
    /// Where the track comes from.
    pub origin: Origin,
}

/// Where a track comes from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Origin {
    /// The media itself.
    #[default]
    Media,
    /// A subtitle file found beside the video or added by the user.
    File,
}

impl Track {
    /// A track known only by its id and kind; the backend fills in what it learns.
    pub fn new(id: TrackId, kind: TrackKind) -> Self {
        Self {
            id,
            kind,
            title: None,
            language: None,
            codec: None,
            channels: None,
            default: false,
            forced: false,
            origin: Origin::Media,
        }
    }
}

/// Where cycling a kind of track (B, V) lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cycled {
    /// The media has no track of that kind.
    NoTracks,
    /// Turned off (subtitles only).
    Off,
    /// This track plays now.
    Track(TrackId),
}

/// All tracks of the media and the ones playing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tracks {
    /// Every track, in the container's order.
    pub all: Vec<Track>,
    /// The playing audio track, or `None` when audio is off.
    pub audio: Option<TrackId>,
    /// The playing video track, or `None` when video is off.
    pub video: Option<TrackId>,
    /// The shown subtitle track, or `None` when subtitles are off.
    pub subtitle: Option<TrackId>,
}

impl Tracks {
    /// The selected track of `kind`.
    pub fn selected(&self, kind: TrackKind) -> Option<TrackId> {
        match kind {
            TrackKind::Audio => self.audio,
            TrackKind::Video => self.video,
            TrackKind::Subtitle => self.subtitle,
        }
    }

    /// Records `track` as the selection for `kind`.
    pub fn select(&mut self, kind: TrackKind, track: Option<TrackId>) {
        match kind {
            TrackKind::Audio => self.audio = track,
            TrackKind::Video => self.video = track,
            TrackKind::Subtitle => self.subtitle = track,
        }
    }

    /// The tracks of `kind`, in the container's order.
    pub fn of(&self, kind: TrackKind) -> impl Iterator<Item = &Track> {
        self.all.iter().filter(move |track| track.kind == kind)
    }

    /// The track with `id`.
    pub fn get(&self, id: TrackId) -> Option<&Track> {
        self.all.iter().find(|track| track.id == id)
    }

    /// The selected track of `kind`, with its details.
    pub fn selected_track(&self, kind: TrackKind) -> Option<&Track> {
        self.selected(kind).and_then(|id| self.get(id))
    }

    /// What menus and the on-screen display call the track with `id`: its title, or a name
    /// built from its language, codec and channels ("English — AC3 5.1"), or "Track N" when
    /// nothing is known.
    pub fn label(&self, id: TrackId) -> Option<String> {
        let track = self.get(id)?;
        let number = self.of(track.kind).position(|other| other.id == id).unwrap_or_default() + 1;
        Some(label::label(track, number))
    }

    /// The selection after the current one for `kind`, wrapping around. Subtitles pass through
    /// "off" after the last track; audio and video don't turn off.
    pub fn cycled(&self, kind: TrackKind) -> Cycled {
        let off = (kind == TrackKind::Subtitle).then_some(None);
        let order: Vec<Option<TrackId>> =
            off.into_iter().chain(self.of(kind).map(|track| Some(track.id))).collect();
        if order.iter().all(Option::is_none) {
            return Cycled::NoTracks;
        }
        let first_track = usize::from(order[0].is_none());
        let next = match order.iter().position(|id| *id == self.selected(kind)) {
            Some(index) => (index + 1) % order.len(),
            // The selection isn't listed (yet): start at the first track.
            None => first_track,
        };
        match order[next] {
            Some(id) => Cycled::Track(id),
            None => Cycled::Off,
        }
    }
}

/// A subtitle file of the open media: found beside the video (S6.1), or added by the user (a
/// drop, or "+" in the track popover).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubtitleFile {
    /// Where the file is.
    pub path: PathBuf,
    /// The subtitle track it became, once the backend lists it.
    pub track: Option<TrackId>,
    /// The language its name gives, for a track the file doesn't tag.
    pub language: Option<Language>,
    /// Its name marks it forced.
    pub forced: bool,
    /// The encoding its text is read in, when known (S6.1.6).
    pub encoding: Option<&'static Encoding>,
    /// The user chose [`Self::encoding`] for this file, rather than it being guessed.
    pub encoding_chosen: bool,
}

impl SubtitleFile {
    /// A file the user just added, whose track isn't listed yet.
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            track: None,
            language: None,
            forced: false,
            encoding: None,
            encoding_chosen: false,
        }
    }

    /// A file just found beside the video, whose track isn't listed yet.
    pub fn found(sidecar: Sidecar) -> Self {
        let Sidecar { path, language, forced } = sidecar;
        Self { language, forced, ..Self::new(path) }
    }

    /// The file's name, as the track popover lists it.
    pub fn name(&self) -> String {
        self.path.file_name().map_or_else(
            || self.path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
    }
}

/// Gives the files still waiting for their track the subtitle tracks that appeared between
/// `before` and `after`. The backend lists an added file after the media's own streams, so a
/// list that arrives with the media's tracks and the file's together gives the file the last
/// new track: the files waiting longest take the last new tracks, in the order they were added.
pub fn claim_added(files: &mut [SubtitleFile], before: &Tracks, after: &Tracks) {
    let claimed: Vec<TrackId> = files.iter().filter_map(|file| file.track).collect();
    let added: Vec<TrackId> = after
        .of(TrackKind::Subtitle)
        .map(|track| track.id)
        .filter(|id| before.get(*id).is_none() && !claimed.contains(id))
        .collect();
    let waiting = files.iter().filter(|file| file.track.is_none()).count();
    let taken = waiting.min(added.len());
    let waiting = files.iter_mut().filter(|file| file.track.is_none());
    for (file, id) in waiting.zip(&added[added.len() - taken..]) {
        file.track = Some(*id);
    }
}

/// Marks the tracks `files` became as coming from a file, with the language and forced flag
/// their names give. libVLC tags a file's track with the last word of its name
/// (`forced` for `Movie.fr.forced.srt`), so the name's language wins, and a tag that names no
/// language is dropped. The backend lists the tracks afresh each time, so this runs on every
/// list.
pub fn describe_files(files: &[SubtitleFile], tracks: &mut Tracks) {
    for file in files {
        let Some(track) = file.track.and_then(|id| tracks.all.iter_mut().find(|t| t.id == id))
        else {
            continue;
        };
        track.origin = Origin::File;
        track.forced |= file.forced;
        let tagged = track.language.take().filter(|tag| Language::recognize(tag.code()).is_some());
        track.language = file.language.clone().or(tagged);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracks(kind: TrackKind, ids: &[i32], selected: Option<i32>) -> Tracks {
        let mut tracks = Tracks {
            all: ids.iter().map(|&id| Track::new(TrackId(id), kind)).collect(),
            ..Tracks::default()
        };
        tracks.select(kind, selected.map(TrackId));
        tracks
    }

    #[test]
    fn subtitles_cycle_through_off() {
        let cycle =
            |selected| tracks(TrackKind::Subtitle, &[3, 4], selected).cycled(TrackKind::Subtitle);
        assert_eq!(cycle(None), Cycled::Track(TrackId(3)));
        assert_eq!(cycle(Some(3)), Cycled::Track(TrackId(4)));
        assert_eq!(cycle(Some(4)), Cycled::Off);
        assert_eq!(cycle(Some(9)), Cycled::Track(TrackId(3)));
    }

    #[test]
    fn audio_cycles_without_turning_off() {
        let cycle = |selected| tracks(TrackKind::Audio, &[1, 2], selected).cycled(TrackKind::Audio);
        assert_eq!(cycle(Some(1)), Cycled::Track(TrackId(2)));
        assert_eq!(cycle(Some(2)), Cycled::Track(TrackId(1)));
        assert_eq!(cycle(None), Cycled::Track(TrackId(1)));
    }

    #[test]
    fn no_tracks_to_cycle() {
        assert_eq!(
            tracks(TrackKind::Audio, &[], None).cycled(TrackKind::Subtitle),
            Cycled::NoTracks
        );
        assert_eq!(
            tracks(TrackKind::Audio, &[1], None).cycled(TrackKind::Subtitle),
            Cycled::NoTracks
        );
    }

    #[test]
    fn untagged_tracks_are_numbered_within_their_kind() {
        let mut all = tracks(TrackKind::Audio, &[1], None);
        all.all.extend(tracks(TrackKind::Subtitle, &[5, 6], None).all);
        assert_eq!(all.label(TrackId(6)).as_deref(), Some("Track 2"));
        assert_eq!(all.label(TrackId(7)), None);
    }

    #[test]
    fn added_files_take_the_last_new_subtitle_tracks() {
        let before = tracks(TrackKind::Subtitle, &[3], None);
        let after = tracks(TrackKind::Subtitle, &[3, 4, 8, 9], None);
        let mut files = vec![
            SubtitleFile { track: Some(TrackId(4)), ..SubtitleFile::new("a.srt".into()) },
            SubtitleFile::new("b.srt".into()),
        ];
        claim_added(&mut files, &before, &after);
        assert_eq!(files[1].track, Some(TrackId(9)));
        assert_eq!(files[1].name(), "b.srt");
        // Nothing new: the waiting file keeps waiting.
        let mut waiting = vec![SubtitleFile::new("c.srt".into())];
        claim_added(&mut waiting, &after, &after);
        assert_eq!(waiting[0].track, None);
    }

    #[test]
    fn a_files_name_gives_its_tracks_language() {
        let mut list = tracks(TrackKind::Subtitle, &[3, 4, 5, 6], None);
        list.all[1].language = Language::parse("fr");
        list.all[2].language = Language::parse("forced");
        list.all[3].language = Language::parse("de");
        let file = |id, language| SubtitleFile {
            track: Some(TrackId(id)),
            language: Language::parse(language),
            forced: true,
            ..SubtitleFile::new("a.srt".into())
        };
        describe_files(&[file(3, "he"), file(4, "en"), file(5, ""), file(6, "")], &mut list);
        let described: Vec<_> = list
            .all
            .iter()
            .map(|track| (track.language.as_ref().map(Language::code), track.forced, track.origin))
            .collect();
        let forced_file = |code| (code, true, Origin::File);
        assert_eq!(
            described,
            [Some("he"), Some("en"), None, Some("de")].map(forced_file),
            "a tag that is no language goes"
        );
    }
}
