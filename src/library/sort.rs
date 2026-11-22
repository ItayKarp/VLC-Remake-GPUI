//! How the library's cards compare under each [`Sort`] (S4.7.1, S4.7.3).
//!
//! Text compares naturally (`E2` before `E10`) and ignoring case. Something unknown (no artist
//! tag, no track number, a file the library hasn't added yet) goes last in both directions
//! (S4.7 edge case). A disc without a number is the first, as in S4.6.2. Whatever a sort leaves
//! tied goes by file name, then by full path, so the order is always the same (S4.7.3).

use std::cmp::Ordering;

use super::folder_view::Card;
use super::watch_state::WatchState;
use crate::navigation::sort::Sort;
use crate::opening::scan::natural_cmp;

/// Which way a sort runs.
#[derive(Clone, Copy)]
enum Direction {
    Ascending,
    Descending,
}

impl Direction {
    fn apply(self, order: Ordering) -> Ordering {
        match self {
            Self::Ascending => order,
            Self::Descending => order.reverse(),
        }
    }
}

/// Orders `cards` by `sort`.
pub fn sort_cards(cards: &mut [Card], sort: Sort) {
    cards.sort_by(|a, b| compare(sort, a, b));
}

/// How two cards compare under `sort`, ties broken by file name, then path.
pub fn compare(sort: Sort, a: &Card, b: &Card) -> Ordering {
    primary(sort, a, b)
        .then_with(|| natural_cmp(&file_name(a), &file_name(b)))
        .then_with(|| a.path.cmp(&b.path))
}

fn primary(sort: Sort, a: &Card, b: &Card) -> Ordering {
    use Direction::{Ascending, Descending};
    match sort {
        Sort::UnwatchedFirst => watch_rank(a).cmp(&watch_rank(b)),
        Sort::WatchedFirst => watch_rank(b).cmp(&watch_rank(a)),
        Sort::NewestFirst => known_last(a.added_ms, b.added_ms, Descending, Ord::cmp),
        Sort::OldestFirst => known_last(a.added_ms, b.added_ms, Ascending, Ord::cmp),
        Sort::TitleAscending => natural_cmp(&a.label, &b.label),
        Sort::TitleDescending => natural_cmp(&b.label, &a.label),
        Sort::ArtistAscending => by_artist(a, b, Ascending),
        Sort::ArtistDescending => by_artist(a, b, Descending),
        Sort::AlbumAscending => by_album(a, b, Ascending),
        Sort::AlbumDescending => by_album(a, b, Descending),
        Sort::TrackAscending => by_track(a, b, Ascending),
        Sort::TrackDescending => by_track(a, b, Descending),
        Sort::UriAscending => natural_cmp(&uri(a), &uri(b)),
        Sort::UriDescending => natural_cmp(&uri(b), &uri(a)),
        Sort::DiscAscending => by_disc(a, b, Ascending),
        Sort::DiscDescending => by_disc(a, b, Descending),
    }
}

/// Unwatched before in progress before watched.
fn watch_rank(card: &Card) -> u8 {
    match card.watch {
        WatchState::Unwatched => 0,
        WatchState::InProgress(_) => 1,
        WatchState::Watched => 2,
    }
}

/// `a` and `b` compared `direction`-wise by `order`, an unknown one after every known one.
fn known_last<T>(
    a: Option<T>,
    b: Option<T>,
    direction: Direction,
    order: impl Fn(&T, &T) -> Ordering,
) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => direction.apply(order(&a, &b)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn text(a: &&str, b: &&str) -> Ordering {
    natural_cmp(a, b)
}

/// By artist, then each artist's albums in order.
fn by_artist(a: &Card, b: &Card, direction: Direction) -> Ordering {
    known_last(artist(a), artist(b), direction, text)
        .then_with(|| by_album(a, b, Direction::Ascending))
}

/// By album, then each album's tracks in order.
fn by_album(a: &Card, b: &Card, direction: Direction) -> Ordering {
    let (a_album, b_album) = (a.tags.album.as_deref(), b.tags.album.as_deref());
    known_last(a_album, b_album, direction, text).then_with(|| by_disc(a, b, Direction::Ascending))
}

/// By disc, then each disc's tracks in order.
fn by_disc(a: &Card, b: &Card, direction: Direction) -> Ordering {
    direction.apply(disc(a).cmp(&disc(b))).then_with(|| by_track(a, b, Direction::Ascending))
}

fn by_track(a: &Card, b: &Card, direction: Direction) -> Ordering {
    known_last(a.tags.track_number, b.tags.track_number, direction, Ord::cmp)
}

/// The artist tag, else the album artist (as [`display::artist`](super::display::artist)
/// shows it).
fn artist(card: &Card) -> Option<&str> {
    card.tags.artist.as_deref().or(card.tags.album_artist.as_deref())
}

fn disc(card: &Card) -> u32 {
    card.tags.disc_number.unwrap_or(1)
}

fn uri(card: &Card) -> String {
    card.path.to_string_lossy().into_owned()
}

fn file_name(card: &Card) -> String {
    card.path.file_name().unwrap_or_default().to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests;
