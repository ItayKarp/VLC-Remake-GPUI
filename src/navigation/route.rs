//! Routes (S4.5.1): every place the library can show, and the rail section each belongs to.

use std::path::PathBuf;

use crate::db::playlists::PlaylistId;
use crate::db::podcasts::FeedId;

/// A place in the library.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    /// What was watched and listened to lately (S4.9).
    Home,
    /// One of Home's sections in full (its `>`).
    HomeSection(HomeSection),
    /// The video folders: their list (`None`), or a folder in one of them.
    Videos(Option<PathBuf>),
    /// The music: its categories, or a place in one.
    Music(MusicRoute),
    /// Network streams: their categories (`None`), or one category (S4.11).
    Network(Option<NetworkCategory>),
    /// A podcast's episodes, in Network → Podcasts (S8.17.2).
    Podcast(FeedId),
    /// The saved playlists (`None`), or one of them.
    Playlists(Option<PlaylistId>),
    /// The recently played list (S4.5.7).
    History,
    /// Search results (S4.7).
    Search(Search),
}

/// Home's sections, top to bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HomeSection {
    /// Videos left partway, the most recent first.
    ContinueWatching,
    /// Audio played lately.
    ListenAgain,
    /// Favorite media and playlists (S4.8.5).
    Favorites,
    /// The library folders visited most.
    Folders,
    /// The saved playlists visited most.
    Playlists,
    /// The artists played most.
    Artists,
}

impl HomeSection {
    /// Every section, in Home's order.
    pub const ALL: [Self; 6] = [
        Self::ContinueWatching,
        Self::ListenAgain,
        Self::Favorites,
        Self::Folders,
        Self::Playlists,
        Self::Artists,
    ];

    /// The section's heading.
    pub fn label(self) -> &'static str {
        match self {
            Self::ContinueWatching => "Continue Watching",
            Self::ListenAgain => "Listen Again",
            Self::Favorites => "Favorites",
            Self::Folders => "Frequently Visited Folders",
            Self::Playlists => "Frequently Visited Playlists",
            Self::Artists => "Frequently Listened Artists",
        }
    }
}

/// Network's categories (S4.11.3): built in, since streams are filed into them rather than
/// organized freely (playlists do that). Podcasts lists subscribed feeds (S8.17.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NetworkCategory {
    /// The streams opened from Open Network Stream, the latest first.
    Recent,
    /// Podcast feeds.
    Podcasts,
    /// Radio stations.
    Radio,
}

impl NetworkCategory {
    /// Every category, in the order Network lists them.
    pub const ALL: [Self; 3] = [Self::Recent, Self::Podcasts, Self::Radio];

    /// The category's name.
    pub fn label(self) -> &'static str {
        match self {
            Self::Recent => "Recent Streams",
            Self::Podcasts => "Podcasts",
            Self::Radio => "Radio",
        }
    }
}

/// A place in the Music section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MusicRoute {
    /// The categories: Songs, Albums, Artists, Genres (`sidebar-music`).
    Categories,
    /// Songs: the music folders (`None`), or a folder in one of them (`sidebar-music2/3`).
    Songs(Option<PathBuf>),
    /// Albums, or one album by name.
    Albums(Option<String>),
    /// Artists, or one artist by name.
    Artists(Option<String>),
    /// Genres, or one genre by name.
    Genres(Option<String>),
}

/// What a search looks for, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Search {
    /// The text typed.
    pub query: String,
    /// The section searched; [`Section::Home`] searches everything.
    pub scope: Section,
}

/// The rail's sections, top to bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    /// Home.
    Home,
    /// Videos.
    Videos,
    /// Music.
    Music,
    /// Network streams.
    Network,
    /// Playlists.
    Playlists,
    /// History.
    History,
}

impl Section {
    /// Every section, in the rail's order.
    pub const ALL: [Self; 6] =
        [Self::Home, Self::Videos, Self::Music, Self::Network, Self::Playlists, Self::History];

    /// The section's name, as its root's breadcrumb and the rail's tooltip say it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Videos => "Videos",
            Self::Music => "Music",
            Self::Network => "Network",
            Self::Playlists => "Playlists",
            Self::History => "History",
        }
    }

    /// The section's root, where its rail icon goes.
    pub fn root(self) -> Route {
        match self {
            Self::Home => Route::Home,
            Self::Videos => Route::Videos(None),
            Self::Music => Route::Music(MusicRoute::Categories),
            Self::Network => Route::Network(None),
            Self::Playlists => Route::Playlists(None),
            Self::History => Route::History,
        }
    }
}

impl MusicRoute {
    /// The category this place is in, if it is in one.
    pub fn category(&self) -> Option<MusicCategory> {
        match self {
            Self::Categories => None,
            Self::Songs(_) => Some(MusicCategory::Songs),
            Self::Albums(_) => Some(MusicCategory::Albums),
            Self::Artists(_) => Some(MusicCategory::Artists),
            Self::Genres(_) => Some(MusicCategory::Genres),
        }
    }
}

/// Music's categories, in the order `sidebar-music` lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusicCategory {
    /// Songs, by folder.
    Songs,
    /// Albums.
    Albums,
    /// Artists.
    Artists,
    /// Genres.
    Genres,
}

impl MusicCategory {
    /// Every category, in order.
    pub const ALL: [Self; 4] = [Self::Songs, Self::Albums, Self::Artists, Self::Genres];

    /// The category's name.
    pub fn label(self) -> &'static str {
        match self {
            Self::Songs => "Songs",
            Self::Albums => "Albums",
            Self::Artists => "Artists",
            Self::Genres => "Genres",
        }
    }

    /// The category's top level.
    pub fn route(self) -> Route {
        Route::Music(match self {
            Self::Songs => MusicRoute::Songs(None),
            Self::Albums => MusicRoute::Albums(None),
            Self::Artists => MusicRoute::Artists(None),
            Self::Genres => MusicRoute::Genres(None),
        })
    }
}

impl Route {
    /// The rail section this route is in. A search is in the section it searches.
    pub fn section(&self) -> Section {
        match self {
            Self::Home | Self::HomeSection(_) => Section::Home,
            Self::Videos(_) => Section::Videos,
            Self::Music(_) => Section::Music,
            Self::Network(_) | Self::Podcast(_) => Section::Network,
            Self::Playlists(_) => Section::Playlists,
            Self::History => Section::History,
            Self::Search(search) => search.scope,
        }
    }

    /// Whether this is its section's root (the rail draws its icon orange there).
    pub fn is_section_root(&self) -> bool {
        *self == self.section().root()
    }

    /// The folder this route shows, for the routes that show one.
    pub fn folder(&self) -> Option<&PathBuf> {
        match self {
            Self::Videos(Some(folder)) | Self::Music(MusicRoute::Songs(Some(folder))) => {
                Some(folder)
            }
            _ => None,
        }
    }

    /// The same kind of route at `folder`: a Videos folder stays in Videos, a Songs folder in
    /// Songs. `None` for routes that show no folder.
    pub fn with_folder(&self, folder: PathBuf) -> Option<Route> {
        match self {
            Self::Videos(_) => Some(Self::Videos(Some(folder))),
            Self::Music(MusicRoute::Songs(_)) => Some(Self::Music(MusicRoute::Songs(Some(folder)))),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_section_root_is_its_own_root() {
        for section in Section::ALL {
            let root = section.root();
            assert_eq!(root.section(), section);
            assert!(root.is_section_root(), "{root:?}");
        }
    }

    #[test]
    fn places_inside_a_section_are_not_its_root() {
        let inside = [
            Route::Videos(Some("/media/Shows".into())),
            MusicCategory::Songs.route(),
            Route::Playlists(Some(PlaylistId(3))),
            Route::Podcast(FeedId(2)),
            Route::Search(Search { query: "wand".into(), scope: Section::Videos }),
            Route::HomeSection(HomeSection::ListenAgain),
        ];
        for route in inside {
            assert!(!route.is_section_root(), "{route:?}");
        }
    }

    #[test]
    fn a_search_belongs_to_the_section_it_searches() {
        let search = Route::Search(Search { query: "x".into(), scope: Section::Music });
        assert_eq!(search.section(), Section::Music);
    }

    #[test]
    fn a_folder_route_keeps_its_kind() {
        let songs = MusicCategory::Songs.route();
        assert_eq!(
            songs.with_folder("/m/My Songs".into()),
            Some(Route::Music(MusicRoute::Songs(Some("/m/My Songs".into()))))
        );
        assert_eq!(Route::Home.with_folder("/m".into()), None);
    }
}
