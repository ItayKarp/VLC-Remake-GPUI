//! The library's sort orders (S4.7.1, S4.7.2; `sorting_options`): the sixteen of the design,
//! which of them each kind of listing offers, its default, and the sort chosen for each place,
//! remembered across launches.
//!
//! Only routes that list media cards sort: a Videos or Songs folder, and one album, artist or
//! genre. A place remembers its own sort (D10: per folder), and only while it isn't the default,
//! so the settings file holds just the places someone re-sorted. Sorts that mean nothing in a
//! place are hidden there: no Artist or Album sorts in a video folder, no Album sorts inside an
//! album. How cards compare is [`crate::library::sort`].

use std::collections::BTreeMap;

use super::route::{MusicRoute, Route};
use super::stored;

/// One way to order a listing's cards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    /// Unwatched, then in progress, then watched.
    UnwatchedFirst,
    /// Watched, then in progress, then unwatched.
    WatchedFirst,
    /// Most recently added to the library first.
    NewestFirst,
    /// Least recently added to the library first.
    OldestFirst,
    /// By what the card says, A to Z.
    TitleAscending,
    /// By what the card says, Z to A.
    TitleDescending,
    /// By artist A to Z, then album, disc and track.
    ArtistAscending,
    /// By artist Z to A, then album, disc and track.
    ArtistDescending,
    /// By album A to Z, then disc and track.
    AlbumAscending,
    /// By album Z to A, then disc and track.
    AlbumDescending,
    /// By track number, lowest first.
    TrackAscending,
    /// By track number, highest first.
    TrackDescending,
    /// By the file's full path, A to Z.
    UriAscending,
    /// By the file's full path, Z to A.
    UriDescending,
    /// By disc number, lowest first, then track.
    DiscAscending,
    /// By disc number, highest first, then track.
    DiscDescending,
}

impl Sort {
    /// Every sort, in the menu's order.
    pub const ALL: [Self; 16] = [
        Self::UnwatchedFirst,
        Self::WatchedFirst,
        Self::NewestFirst,
        Self::OldestFirst,
        Self::TitleAscending,
        Self::TitleDescending,
        Self::ArtistAscending,
        Self::ArtistDescending,
        Self::AlbumAscending,
        Self::AlbumDescending,
        Self::TrackAscending,
        Self::TrackDescending,
        Self::UriAscending,
        Self::UriDescending,
        Self::DiscAscending,
        Self::DiscDescending,
    ];

    /// What the menu calls it.
    pub fn label(self) -> &'static str {
        self.names().0
    }

    /// Its name in the settings file.
    fn key(self) -> &'static str {
        self.names().1
    }

    fn names(self) -> (&'static str, &'static str) {
        match self {
            Self::UnwatchedFirst => ("Unwatched First", "unwatched-first"),
            Self::WatchedFirst => ("Watched First", "watched-first"),
            Self::NewestFirst => ("Newest First", "newest-first"),
            Self::OldestFirst => ("Oldest First", "oldest-first"),
            Self::TitleAscending => ("Title A-Z", "title"),
            Self::TitleDescending => ("Title Z-A", "title-descending"),
            Self::ArtistAscending => ("Artist A-Z", "artist"),
            Self::ArtistDescending => ("Artist Z-A", "artist-descending"),
            Self::AlbumAscending => ("Album A-Z", "album"),
            Self::AlbumDescending => ("Album Z-A", "album-descending"),
            Self::TrackAscending => ("Track Number Ascending", "track"),
            Self::TrackDescending => ("Track Number Descending", "track-descending"),
            Self::UriAscending => ("URI Ascending", "uri"),
            Self::UriDescending => ("URI Descending", "uri-descending"),
            Self::DiscAscending => ("Disc Number Ascending", "disc"),
            Self::DiscDescending => ("Disc Number Descending", "disc-descending"),
        }
    }

    /// What hovering it in the menu explains, if anything: "newest" is the date added to the
    /// library, not the file's date (S4.7 edge case).
    pub fn hint(self) -> Option<&'static str> {
        match self {
            Self::NewestFirst | Self::OldestFirst => Some("By the date added to the library"),
            _ => None,
        }
    }

    /// Whether it orders names Z to A, which turns a folder's subfolders around too. The other
    /// sorts leave subfolders in natural order: they have no tags or history.
    pub fn reverses_names(self) -> bool {
        matches!(self, Self::TitleDescending | Self::UriDescending)
    }

    fn named(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|sort| sort.key() == key)
    }
}

/// A kind of listing that sorts, which decides its sorts and its default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sortable {
    /// A Videos folder.
    VideoFolder,
    /// A Songs folder.
    SongFolder,
    /// One album's songs.
    Album,
    /// One artist's songs.
    Artist,
    /// One genre's songs.
    Genre,
}

impl Sortable {
    /// What kind of listing `route` is, if it sorts.
    pub fn of(route: &Route) -> Option<Self> {
        match route {
            Route::Videos(Some(_)) => Some(Self::VideoFolder),
            Route::Music(MusicRoute::Songs(Some(_))) => Some(Self::SongFolder),
            Route::Music(MusicRoute::Albums(Some(_))) => Some(Self::Album),
            Route::Music(MusicRoute::Artists(Some(_))) => Some(Self::Artist),
            Route::Music(MusicRoute::Genres(Some(_))) => Some(Self::Genre),
            _ => None,
        }
    }

    /// The sorts it offers, in the menu's order.
    pub fn options(self) -> Vec<Sort> {
        Sort::ALL.into_iter().filter(|sort| self.offers(*sort)).collect()
    }

    fn offers(self, sort: Sort) -> bool {
        use Sort::{AlbumAscending, AlbumDescending, ArtistAscending, ArtistDescending};
        let artist = matches!(sort, ArtistAscending | ArtistDescending);
        let album = matches!(sort, AlbumAscending | AlbumDescending);
        match self {
            Self::VideoFolder => !artist && !album && !sort.is_music_only(),
            Self::Album => !album,
            Self::Artist => !artist,
            Self::SongFolder | Self::Genre => true,
        }
    }

    /// How it sorts until someone picks another: folders by name, an album by disc and track,
    /// an artist by album, a genre by artist (S4.6.2's orders).
    pub fn default_sort(self) -> Sort {
        match self {
            Self::VideoFolder | Self::SongFolder => Sort::TitleAscending,
            Self::Album => Sort::DiscAscending,
            Self::Artist => Sort::AlbumAscending,
            Self::Genre => Sort::ArtistAscending,
        }
    }
}

impl Sort {
    /// Whether it orders by a tag only music has (track and disc numbers).
    fn is_music_only(self) -> bool {
        matches!(
            self,
            Self::TrackAscending
                | Self::TrackDescending
                | Self::DiscAscending
                | Self::DiscDescending
        )
    }
}

/// How `route` sorts, given the sorts remembered per place; `None` when it doesn't sort. A
/// remembered sort the place doesn't offer (a hand edit) reads as its default.
pub fn of(route: &Route, remembered: &BTreeMap<String, String>) -> Option<Sort> {
    let sortable = Sortable::of(route)?;
    let chosen = remembered.get(&stored::encode(route)).and_then(|key| Sort::named(key));
    Some(chosen.filter(|sort| sortable.offers(*sort)).unwrap_or(sortable.default_sort()))
}

/// Remembers `sort` as how `route` sorts; its default is remembered by forgetting the place.
pub fn remember(route: &Route, sort: Sort, remembered: &mut BTreeMap<String, String>) {
    let Some(sortable) = Sortable::of(route) else { return };
    let place = stored::encode(route);
    if sort == sortable.default_sort() {
        remembered.remove(&place);
    } else {
        remembered.insert(place, sort.key().to_owned());
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn folder(path: &str) -> Route {
        Route::Videos(Some(PathBuf::from(path)))
    }

    #[test]
    fn each_place_remembers_its_own_sort_and_forgets_the_default() {
        let mut remembered = BTreeMap::new();
        let (shows, films) = (folder("/v/Shows"), folder("/v/Films"));
        assert_eq!(of(&shows, &remembered), Some(Sort::TitleAscending));
        remember(&shows, Sort::NewestFirst, &mut remembered);
        assert_eq!(of(&shows, &remembered), Some(Sort::NewestFirst));
        assert_eq!(of(&films, &remembered), Some(Sort::TitleAscending));
        remember(&shows, Sort::TitleAscending, &mut remembered);
        assert!(remembered.is_empty());
    }

    #[test]
    fn only_card_listings_sort() {
        assert_eq!(of(&Route::Videos(None), &BTreeMap::new()), None);
        assert_eq!(of(&Route::Music(MusicRoute::Albums(None)), &BTreeMap::new()), None);
        let album = Route::Music(MusicRoute::Albums(Some("Juxt".into())));
        assert_eq!(of(&album, &BTreeMap::new()), Some(Sort::DiscAscending));
    }

    #[test]
    fn places_hide_the_sorts_that_mean_nothing_there() {
        let video = Sortable::VideoFolder.options();
        assert_eq!(video.len(), 8);
        assert!(!video.contains(&Sort::ArtistAscending) && !video.contains(&Sort::DiscAscending));
        assert_eq!(Sortable::SongFolder.options(), Sort::ALL);
        assert!(!Sortable::Album.options().contains(&Sort::AlbumDescending));
        assert!(!Sortable::Artist.options().contains(&Sort::ArtistAscending));
    }

    #[test]
    fn a_sort_the_place_does_not_offer_reads_as_its_default() {
        let shows = folder("/v/Shows");
        let mut remembered = BTreeMap::new();
        remembered.insert(stored::encode(&shows), "artist".to_owned());
        assert_eq!(of(&shows, &remembered), Some(Sort::TitleAscending));
        remembered.insert(stored::encode(&shows), "nonsense".to_owned());
        assert_eq!(of(&shows, &remembered), Some(Sort::TitleAscending));
    }

    #[test]
    fn every_sort_has_its_own_name() {
        for sort in Sort::ALL {
            assert_eq!(Sort::named(sort.key()), Some(sort));
        }
    }
}
