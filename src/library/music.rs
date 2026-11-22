//! Music's Albums, Artists and Genres (S4.6.2): the names each category lists, and the songs
//! under one name, as cards.
//!
//! Names come from the tags with [`display`]'s fallbacks ("Unknown Album", "Unknown Artist",
//! "Unknown Genre"), compare ignoring case, and list in natural order with the unknown one last.
//! A tag naming several artists (`A feat. B`) lists the song under each.
//!
//! An album is its name: two albums of the same name by different artists list as one. An
//! album's tracks are in disc and track order; an artist's songs go by album, then disc and
//! track; a genre's by artist, then the same.

use std::cmp::Ordering;
use std::collections::HashMap;

use super::display::{self, ARTIST_SEPARATORS, UNKNOWN_ALBUM, UNKNOWN_ARTIST};
use super::folder_view::Card;
use super::watch_state::WatchState;
use crate::db::metadata::{MediaKind, Metadata};
use crate::db::songs::Song;
use crate::history::policy::WatchedRule;
use crate::opening::scan::natural_cmp;

/// Shown for a song without a genre tag.
pub const UNKNOWN_GENRE: &str = "Unknown Genre";

/// How Music groups songs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grouping {
    /// By album.
    Albums,
    /// By artist, each of a tag's artists.
    Artists,
    /// By genre.
    Genres,
}

impl Grouping {
    /// The names `song` lists under.
    fn names(self, song: &Song) -> Vec<String> {
        let tags = &song.metadata;
        match self {
            Self::Albums => vec![display::album(tags).to_owned()],
            Self::Artists => {
                let artists = display::split_artists(display::artist(tags), &ARTIST_SEPARATORS);
                if artists.is_empty() { vec![UNKNOWN_ARTIST.to_owned()] } else { artists }
            }
            Self::Genres => vec![tags.genre.clone().unwrap_or_else(|| UNKNOWN_GENRE.to_owned())],
        }
    }

    /// The name this grouping gives songs without the tag.
    fn unknown(self) -> &'static str {
        match self {
            Self::Albums => UNKNOWN_ALBUM,
            Self::Artists => UNKNOWN_ARTIST,
            Self::Genres => UNKNOWN_GENRE,
        }
    }
}

/// Every name `songs` list under when grouped `by`, each once, in natural order, the unknown
/// one last.
pub fn names(songs: &[Song], by: Grouping) -> Vec<String> {
    let mut first_seen: HashMap<String, String> = HashMap::new();
    for name in songs.iter().flat_map(|song| by.names(song)) {
        first_seen.entry(name.to_lowercase()).or_insert(name);
    }
    let unknown = by.unknown().to_lowercase();
    let mut names: Vec<(bool, String)> =
        first_seen.into_iter().map(|(key, name)| (key == unknown, name)).collect();
    names.sort_by(|(a_unknown, a), (b_unknown, b)| {
        a_unknown.cmp(b_unknown).then_with(|| natural_cmp(a, b))
    });
    names.into_iter().map(|(_, name)| name).collect()
}

/// The cards of the songs listed under `name` when grouped `by`, in that grouping's order, with
/// `rule` deciding what is watched.
pub fn songs_in(songs: Vec<Song>, by: Grouping, name: &str, rule: WatchedRule) -> Vec<Card> {
    let name = name.to_lowercase();
    let mut songs: Vec<Song> = songs
        .into_iter()
        .filter(|song| by.names(song).iter().any(|listed| listed.to_lowercase() == name))
        .collect();
    songs.sort_by(|a, b| order(by, a, b));
    songs.into_iter().map(|song| card(song, rule)).collect()
}

/// How two songs under one name compare.
fn order(by: Grouping, a_song: &Song, b_song: &Song) -> Ordering {
    let (a, b) = (&a_song.metadata, &b_song.metadata);
    let by_artist = || natural_cmp(display::artist(a), display::artist(b));
    let by_album = || natural_cmp(display::album(a), display::album(b));
    let within_album = || {
        disc(a)
            .cmp(&disc(b))
            .then_with(|| track(a).cmp(&track(b)))
            .then_with(|| natural_cmp(&title(a_song), &title(b_song)))
    };
    match by {
        Grouping::Albums => within_album(),
        Grouping::Artists => by_album().then_with(within_album),
        Grouping::Genres => by_artist().then_with(by_album).then_with(within_album),
    }
}

/// A song's disc: untagged is the first.
fn disc(tags: &Metadata) -> u32 {
    tags.disc_number.unwrap_or(1)
}

/// A song's track: untagged ones come after the numbered ones.
fn track(tags: &Metadata) -> u32 {
    tags.track_number.unwrap_or(u32::MAX)
}

fn title(song: &Song) -> String {
    display::title(&song.metadata, &song.path)
}

fn card(song: Song, rule: WatchedRule) -> Card {
    Card {
        label: title(&song),
        media: Some(song.media),
        kind: MediaKind::Audio,
        watch: WatchState::of(&song.saved, rule),
        path: song.path,
    }
}

#[cfg(test)]
mod tests;
