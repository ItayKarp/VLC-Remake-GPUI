//! Favorites (S4.8.1, S4.8.3): the media and playlists the file and playlist menus' Favorite
//! marks. The [`Favorites`] global holds them in memory, so a menu knows at once whether its
//! items are favorites; a change shows at once and is written on the history thread. Home's
//! Favorites row (S4.8.5, S4.9) reads the same global.

use std::collections::HashSet;

use futures::channel::oneshot;
use gpui::{App, BorrowAppContext as _, Global};

use crate::db::activity::{self, Favorite};
use crate::db::now_ms;
use crate::history::History;

/// The favorites, as the menus show them.
#[derive(Default)]
pub struct Favorites {
    marked: HashSet<Favorite>,
}

impl Global for Favorites {}

/// Starts with no favorites and reads them from the database.
pub fn init(cx: &mut App) {
    cx.set_global(Favorites::default());
    let Some(history) = History::get(cx) else { return };
    let (reply, answer) = oneshot::channel();
    history.with_database(move |connection| {
        let read = activity::favorites(connection).unwrap_or_else(|error| {
            tracing::warn!("library: can't read the favorites: {error}");
            Vec::new()
        });
        let _ = reply.send(read);
    });
    cx.spawn(async move |cx| {
        let Ok(read) = answer.await else { return };
        cx.update(|cx| cx.update_global::<Favorites, _>(|global, _| global.marked.extend(read)));
    })
    .detach();
}

/// Whether every one of `favorites` is a favorite; `false` for none.
pub fn all(favorites: &[Favorite], cx: &App) -> bool {
    let Some(global) = cx.try_global::<Favorites>() else { return false };
    !favorites.is_empty() && favorites.iter().all(|favorite| global.marked.contains(favorite))
}

/// Makes `favorites` favorites (`marked`) or stops them being ones.
pub fn mark(favorites: Vec<Favorite>, marked: bool, cx: &mut App) {
    if !cx.has_global::<Favorites>() {
        return;
    }
    cx.update_global::<Favorites, _>(|global, _| {
        for favorite in &favorites {
            if marked {
                global.marked.insert(*favorite);
            } else {
                global.marked.remove(favorite);
            }
        }
    });
    let Some(history) = History::get(cx) else { return };
    history.with_database(move |connection| {
        let now = now_ms();
        for favorite in favorites {
            let written = if marked {
                activity::add_favorite(connection, favorite, now)
            } else {
                activity::remove_favorite(connection, favorite)
            };
            if let Err(error) = written {
                tracing::warn!("library: can't save a favorite: {error}");
            }
        }
    });
}
