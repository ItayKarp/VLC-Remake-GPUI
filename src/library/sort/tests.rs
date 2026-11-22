use std::path::PathBuf;

use super::*;
use crate::db::metadata::{MediaKind, Metadata};

fn card(name: &str) -> Card {
    Card {
        label: name.to_owned(),
        path: PathBuf::from(format!("/m/{name}")),
        media: None,
        kind: MediaKind::Audio,
        watch: WatchState::Unwatched,
        tags: Metadata::default(),
        added_ms: None,
    }
}

fn tagged(name: &str, artist: Option<&str>, album: Option<&str>, track: Option<u32>) -> Card {
    let tags = Metadata {
        artist: artist.map(Into::into),
        album: album.map(Into::into),
        track_number: track,
        ..Metadata::default()
    };
    Card { tags, ..card(name) }
}

fn sorted(mut cards: Vec<Card>, sort: Sort) -> Vec<String> {
    sort_cards(&mut cards, sort);
    cards.into_iter().map(|card| card.label).collect()
}

#[test]
fn titles_sort_naturally_both_ways() {
    let cards = vec![card("E10"), card("e2"), card("E1")];
    assert_eq!(sorted(cards.clone(), Sort::TitleAscending), ["E1", "e2", "E10"]);
    assert_eq!(sorted(cards, Sort::TitleDescending), ["E10", "e2", "E1"]);
}

#[test]
fn watch_states_group_in_order() {
    let watched = Card { watch: WatchState::Watched, ..card("a") };
    let partly = Card { watch: WatchState::InProgress(0.5), ..card("b") };
    let cards = vec![watched, partly, card("c")];
    assert_eq!(sorted(cards.clone(), Sort::UnwatchedFirst), ["c", "b", "a"]);
    assert_eq!(sorted(cards, Sort::WatchedFirst), ["a", "b", "c"]);
}

#[test]
fn newest_goes_by_date_added_with_unknown_dates_last() {
    let old = Card { added_ms: Some(1), ..card("old") };
    let new = Card { added_ms: Some(2), ..card("new") };
    let cards = vec![card("unadded"), old, new];
    assert_eq!(sorted(cards.clone(), Sort::NewestFirst), ["new", "old", "unadded"]);
    assert_eq!(sorted(cards, Sort::OldestFirst), ["old", "new", "unadded"]);
}

#[test]
fn unknown_artists_go_last_in_both_directions() {
    let cards = vec![
        tagged("x", None, None, None),
        tagged("b1", Some("B"), Some("Album 10"), None),
        tagged("b2", Some("b"), Some("Album 2"), None),
        tagged("a", Some("A"), None, None),
    ];
    assert_eq!(sorted(cards.clone(), Sort::ArtistAscending), ["a", "b2", "b1", "x"]);
    assert_eq!(sorted(cards, Sort::ArtistDescending), ["b2", "b1", "a", "x"]);
}

#[test]
fn albums_list_their_tracks_in_disc_and_track_order() {
    let mut second_disc = tagged("intro 2", None, Some("Juxt"), Some(1));
    second_disc.tags.disc_number = Some(2);
    let cards = vec![
        second_disc,
        tagged("middle", None, Some("Juxt"), Some(2)),
        tagged("intro", None, Some("Juxt"), Some(1)),
        tagged("other", None, Some("Alpha"), None),
    ];
    assert_eq!(
        sorted(cards.clone(), Sort::AlbumAscending),
        ["other", "intro", "middle", "intro 2"]
    );
    assert_eq!(
        sorted(cards.clone(), Sort::AlbumDescending),
        ["intro", "middle", "intro 2", "other"]
    );
    assert_eq!(sorted(cards, Sort::DiscDescending), ["intro 2", "intro", "middle", "other"]);
}

#[test]
fn untagged_tracks_go_last_and_ties_go_by_file_name() {
    let cards = vec![
        tagged("b", None, None, None),
        tagged("a", None, None, None),
        tagged("one", None, None, Some(1)),
        tagged("two", None, None, Some(2)),
    ];
    assert_eq!(sorted(cards.clone(), Sort::TrackAscending), ["one", "two", "a", "b"]);
    assert_eq!(sorted(cards, Sort::TrackDescending), ["two", "one", "a", "b"]);
}

#[test]
fn uris_sort_by_the_whole_path() {
    let deep = Card { path: PathBuf::from("/a/z/file"), ..card("deep") };
    let cards = vec![card("b"), deep];
    assert_eq!(sorted(cards.clone(), Sort::UriAscending), ["deep", "b"]);
    assert_eq!(sorted(cards, Sort::UriDescending), ["b", "deep"]);
}
