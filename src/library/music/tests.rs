use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use super::*;
use crate::db::Database;
use crate::db::files::{self, Scanned};
use crate::db::folders;
use crate::db::media::{self, Entry, FileStamp, MediaId, Saved};
use crate::db::metadata::{self, Metadata};
use crate::db::roots::{self, RootKind};
use crate::db::songs;
use crate::library::path_key::Case;
use crate::library::sort::sort_cards;
use crate::navigation::sort::Sortable;
use crate::settings::Settings;

fn rule() -> WatchedRule {
    WatchedRule::from_settings(&Settings::default().prefs.player)
}

#[derive(Clone, Copy)]
struct Tags {
    title: &'static str,
    artist: Option<&'static str>,
    album: Option<&'static str>,
    genre: Option<&'static str>,
    disc: Option<u32>,
    track: Option<u32>,
}

const BLANK: Tags =
    Tags { title: "", artist: None, album: None, genre: None, disc: None, track: None };

fn song(id: i64, tags: Tags) -> Song {
    let metadata = Metadata {
        kind: MediaKind::Audio,
        title: Some(tags.title.to_owned()),
        artist: tags.artist.map(Into::into),
        album: tags.album.map(Into::into),
        genre: tags.genre.map(Into::into),
        disc_number: tags.disc,
        track_number: tags.track,
        ..Metadata::default()
    };
    let path = PathBuf::from(format!("/m/{id}.mp3"));
    Song { media: MediaId(id), path, metadata, saved: Saved::default(), added_ms: None }
}

fn library() -> Vec<Song> {
    vec![
        song(
            1,
            Tags { title: "Outro", album: Some("Juxt"), disc: Some(2), track: Some(1), ..BLANK },
        ),
        song(
            2,
            Tags { title: "Intro", album: Some("juxt"), disc: Some(1), track: Some(1), ..BLANK },
        ),
        song(3, Tags { title: "Middle", album: Some("Juxt"), track: Some(2), ..BLANK }),
        song(
            4,
            Tags { title: "Loose", artist: Some("B; A feat. C"), genre: Some("Jazz"), ..BLANK },
        ),
        song(5, Tags { title: "Track 10", album: Some("Album 10"), artist: Some("A"), ..BLANK }),
        song(6, Tags { title: "Track 2", album: Some("Album 2"), artist: Some("a"), ..BLANK }),
    ]
}

fn titles(cards: &[Card]) -> Vec<&str> {
    cards.iter().map(|card| card.label.as_str()).collect()
}

/// The songs under `name`, in the order the listing shows them by default.
fn listed(by: Grouping, sortable: Sortable, name: &str) -> Vec<Card> {
    let mut cards = songs_in(library(), by, name, rule());
    sort_cards(&mut cards, sortable.default_sort());
    cards
}

#[test]
fn names_are_listed_once_in_natural_order_with_the_unknown_last() {
    let songs = library();
    assert_eq!(names(&songs, Grouping::Albums), ["Album 2", "Album 10", "Juxt", UNKNOWN_ALBUM]);
    assert_eq!(names(&songs, Grouping::Artists), ["A", "B", "C", UNKNOWN_ARTIST]);
    assert_eq!(names(&songs, Grouping::Genres), ["Jazz", UNKNOWN_GENRE]);
}

#[test]
fn an_album_s_tracks_go_by_disc_and_track() {
    let tracks = listed(Grouping::Albums, Sortable::Album, "JUXT");
    assert_eq!(titles(&tracks), ["Intro", "Middle", "Outro"]);
    assert_eq!(tracks[0].media, Some(MediaId(2)));
}

#[test]
fn an_artist_s_songs_go_by_album_and_include_shared_tags() {
    let songs = listed(Grouping::Artists, Sortable::Artist, "a");
    assert_eq!(titles(&songs), ["Track 2", "Track 10", "Loose"]);
    let unknown = listed(Grouping::Artists, Sortable::Artist, UNKNOWN_ARTIST);
    assert_eq!(titles(&unknown), ["Intro", "Middle", "Outro"]);
}

#[test]
fn a_genre_lists_its_songs() {
    assert_eq!(titles(&songs_in(library(), Grouping::Genres, "jazz", rule())), ["Loose"]);
    assert_eq!(songs_in(library(), Grouping::Genres, "Rock", rule()), []);
}

#[test]
fn the_library_s_songs_are_read_once_each() {
    let mut database = Database::in_memory();
    let c = database.connection();
    roots::add(c, Path::new("/m"), RootKind::Music, Case::Sensitive, 1).expect("add");
    let folder = folders::find(c, Path::new("/m"), Case::Sensitive).expect("find").expect("root");
    let stamp = FileStamp { size: 1, modified_ms: None };
    let add = |identity: &str, kind: MediaKind, names: &[&str]| {
        let entry =
            Entry { identity: identity.into(), location: format!("/m/{identity}"), stamp: None };
        let id = media::insert(c, &entry).expect("insert");
        let tags = Metadata { kind, album: Some("Juxt".into()), ..Metadata::default() };
        metadata::set(c, id, &tags).expect("tags");
        for name in names {
            let file = Scanned { name: OsStr::new(name), stamp, media: Some(id), cloud: false };
            files::put(c, folder, &file, Case::Sensitive).expect("put");
        }
        id
    };
    let song = add("a", MediaKind::Audio, &["a.mp3", "copy of a.mp3"]);
    add("v", MediaKind::Video, &["v.mkv"]);
    let found = songs::all(c).expect("songs");
    assert_eq!(found.len(), 1);
    assert_eq!((found[0].media, found[0].path.clone()), (song, PathBuf::from("/m/a.mp3")));
    assert_eq!(found[0].metadata.album.as_deref(), Some("Juxt"));
}
