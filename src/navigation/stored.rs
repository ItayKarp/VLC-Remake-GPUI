//! Routes as text, so the last one can be saved in `local.toml` (S4.5.6).
//!
//! The form is `kind[/category][:value]`: `home`, `home:listen-again`, `videos`, `videos:/media/Shows`,
//! `music/songs:/music/My Songs`, `music/albums:Juxt`, `playlists:4`, `search/videos:wand`.
//! Paths are encoded like every stored path, so any path survives. Text that doesn't parse (a
//! newer build's route, a hand edit) reads as nothing, and the library opens at Home.

use super::route::{HomeSection, MusicRoute, Route, Search, Section};
use crate::db::playlists::PlaylistId;
use crate::settings::stored_path;

/// `route` as text.
pub fn encode(route: &Route) -> String {
    let path = |path: &std::path::PathBuf| stored_path::encode(path);
    let (kind, value) = match route {
        Route::Home => ("home", None),
        Route::HomeSection(section) => ("home", Some(home_section_name(*section).to_owned())),
        Route::Videos(folder) => ("videos", folder.as_ref().map(path)),
        Route::Music(MusicRoute::Categories) => ("music", None),
        Route::Music(MusicRoute::Songs(folder)) => ("music/songs", folder.as_ref().map(path)),
        Route::Music(MusicRoute::Albums(name)) => ("music/albums", name.clone()),
        Route::Music(MusicRoute::Artists(name)) => ("music/artists", name.clone()),
        Route::Music(MusicRoute::Genres(name)) => ("music/genres", name.clone()),
        Route::Network(category) => ("network", category.clone()),
        Route::Playlists(id) => ("playlists", id.map(|id| id.0.to_string())),
        Route::History => ("history", None),
        Route::Search(search) => {
            return format!("search/{}:{}", scope_name(search.scope), search.query);
        }
    };
    match value {
        Some(value) => format!("{kind}:{value}"),
        None => kind.to_owned(),
    }
}

/// The route `text` names, if it names one.
pub fn decode(text: &str) -> Option<Route> {
    let (kind, value) = match text.split_once(':') {
        Some((kind, value)) => (kind, Some(value)),
        None => (text, None),
    };
    let name = || value.map(str::to_owned);
    let folder = || value.filter(|path| !path.is_empty()).and_then(stored_path::decode);
    let route = match kind {
        "home" => match value {
            None => Route::Home,
            Some(name) => Route::HomeSection(
                HomeSection::ALL.into_iter().find(|section| home_section_name(*section) == name)?,
            ),
        },
        "videos" => Route::Videos(folder()),
        "music" => Route::Music(MusicRoute::Categories),
        "music/songs" => Route::Music(MusicRoute::Songs(folder())),
        "music/albums" => Route::Music(MusicRoute::Albums(name())),
        "music/artists" => Route::Music(MusicRoute::Artists(name())),
        "music/genres" => Route::Music(MusicRoute::Genres(name())),
        "network" => Route::Network(name()),
        "playlists" => Route::Playlists(value.map(str::parse).transpose().ok()?.map(PlaylistId)),
        "history" => Route::History,
        _ => {
            let scope = kind.strip_prefix("search/")?;
            let scope = Section::ALL.into_iter().find(|section| scope_name(*section) == scope)?;
            Route::Search(Search { query: value?.to_owned(), scope })
        }
    };
    // A value the kind can't hold, or a path that doesn't decode, makes the text unreadable.
    (encode(&route) == text).then_some(route)
}

fn home_section_name(section: HomeSection) -> &'static str {
    match section {
        HomeSection::ContinueWatching => "continue-watching",
        HomeSection::ListenAgain => "listen-again",
        HomeSection::Favorites => "favorites",
        HomeSection::Folders => "folders",
        HomeSection::Playlists => "playlists",
        HomeSection::Artists => "artists",
    }
}

fn scope_name(section: Section) -> &'static str {
    match section {
        Section::Home => "all",
        Section::Videos => "videos",
        Section::Music => "music",
        Section::Network => "network",
        Section::Playlists => "playlists",
        Section::History => "history",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::navigation::route::MusicCategory;

    #[test]
    fn every_kind_of_route_survives() {
        let routes = [
            Route::Home,
            Route::HomeSection(HomeSection::ContinueWatching),
            Route::HomeSection(HomeSection::Artists),
            Route::Videos(None),
            Route::Videos(Some("/media/Shows: the best/S01".into())),
            Route::Music(MusicRoute::Categories),
            MusicCategory::Songs.route(),
            Route::Music(MusicRoute::Songs(Some("/music/My Songs".into()))),
            Route::Music(MusicRoute::Albums(Some("Juxt: Live".into()))),
            Route::Music(MusicRoute::Artists(None)),
            Route::Music(MusicRoute::Genres(Some("Jazz".into()))),
            Route::Network(Some("Radio".into())),
            Route::Playlists(Some(PlaylistId(42))),
            Route::Playlists(None),
            Route::History,
            Route::Search(Search { query: "a:b".into(), scope: Section::Home }),
            Route::Search(Search { query: String::new(), scope: Section::Music }),
        ];
        for route in routes {
            assert_eq!(decode(&encode(&route)), Some(route.clone()), "{}", encode(&route));
        }
    }

    #[test]
    fn unreadable_text_is_no_route() {
        let texts =
            ["", "settings", "playlists:x", "search/podcasts:a", "search/all", "videos:", "home:x"];
        for text in texts {
            assert_eq!(decode(text), None, "{text}");
        }
    }

    #[test]
    fn paths_read_plainly() {
        assert_eq!(encode(&Route::Videos(Some("/media/Shows".into()))), "videos:/media/Shows");
    }
}
