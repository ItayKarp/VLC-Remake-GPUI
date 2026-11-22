//! What Home remembers besides plays (S4.9): a visit to a library folder or a saved playlist,
//! which Frequently Visited ranks, and "Remove from Continue Watching" (S4.9.3).
//!
//! Both are written on the history thread. With "Save recently played items" off, visits aren't
//! counted.

use std::path::PathBuf;

use gpui::App;

use crate::db::activity::{self, Place};
use crate::db::media::MediaId;
use crate::db::{folders, now_ms, played};
use crate::history::History;
use crate::library::path_key::Case;
use crate::navigation::route::Route;
use crate::settings;

/// What a visit to a route counts toward: a folder by its path, or a playlist.
enum Visited {
    Folder(PathBuf),
    Place(Place),
}

/// Counts a visit to `route`, if it is a folder or a saved playlist and history is kept. A
/// folder outside the library isn't counted.
pub fn record_visit(route: &Route, cx: &App) {
    if !settings::get(cx).prefs.privacy.save_recently_played {
        return;
    }
    let visited = match route {
        Route::Playlists(Some(id)) => Visited::Place(Place::Playlist(*id)),
        _ => match route.folder() {
            Some(folder) => Visited::Folder(folder.clone()),
            None => return,
        },
    };
    let Some(history) = History::get(cx) else { return };
    history.with_database(move |connection| {
        let place = match visited {
            Visited::Place(place) => Some(place),
            // The exact key first, as the folder views look folders up.
            Visited::Folder(path) => [Case::Sensitive, Case::Insensitive]
                .into_iter()
                .find_map(|case| folders::find(connection, &path, case).ok().flatten())
                .map(Place::Folder),
        };
        let Some(place) = place else { return };
        if let Err(error) = activity::record_visit(connection, place, now_ms()) {
            tracing::warn!("library: can't count a visit: {error}");
        }
    });
}

/// Takes `media` off Continue Watching until each is played again, keeping its progress.
pub fn remove_from_continue_watching(media: Vec<MediaId>, cx: &App) {
    let Some(history) = History::get(cx) else { return };
    history.with_database(move |connection| {
        let now = now_ms();
        for id in media {
            if let Err(error) = played::hide_in_progress(connection, id, now) {
                tracing::warn!("library: can't remove from Continue Watching: {error}");
            }
        }
    });
}
