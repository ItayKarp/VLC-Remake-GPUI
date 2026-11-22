//! Albums, Artists and Genres in the listing (S4.6.2): a category lists its names as rows, and
//! a name lists its songs as cards, both read from the library in the background.

use futures::channel::oneshot;
use gpui::{App, AppContext as _, Context, Task};

use super::{Contents, FolderState, category_icon};
use crate::db::songs::{self, Song};
use crate::history::History;
use crate::history::policy::WatchedRule;
use crate::library::music::{self, Grouping};
use crate::main_window::MainWindow;
use crate::main_window::library_panel::row::{Row, Target};
use crate::navigation::route::{MusicCategory, MusicRoute, Route};
use crate::navigation::sort::Sort;
use crate::settings;

/// Where a Music route is: its grouping, and the name it shows, if one.
fn place(route: &Route) -> Option<(Grouping, Option<&String>)> {
    match route {
        Route::Music(MusicRoute::Albums(name)) => Some((Grouping::Albums, name.as_ref())),
        Route::Music(MusicRoute::Artists(name)) => Some((Grouping::Artists, name.as_ref())),
        Route::Music(MusicRoute::Genres(name)) => Some((Grouping::Genres, name.as_ref())),
        _ => None,
    }
}

/// Reads what `route` lists in the background, for a place in Albums, Artists or Genres, its
/// songs ordered by `sort`; `None` for other routes.
pub(super) fn read(
    route: Route,
    sort: Option<Sort>,
    cx: &mut Context<MainWindow>,
) -> Option<Task<()>> {
    let (grouping, name) = place(&route)?;
    let name = name.cloned();
    let rule = WatchedRule::from_settings(&settings::get(cx).prefs.player);
    let songs = read_songs(cx);
    Some(cx.spawn(async move |this, cx| {
        let songs = songs.await.unwrap_or_default();
        let contents = cx.background_spawn(async move {
            match name {
                Some(name) => {
                    let cards = music::songs_in(songs, grouping, &name, rule);
                    let mut contents = Contents { cards, ..Contents::default() };
                    contents.sort(sort.unwrap_or(Sort::TitleAscending));
                    contents
                }
                None => Contents { names: music::names(&songs, grouping), ..Contents::default() },
            }
        });
        let state = FolderState::Read(contents.await);
        this.update(cx, |this, cx| this.read_into(&route, state, cx)).ok();
    }))
}

/// Every song in the library, read on the history thread.
fn read_songs(cx: &App) -> oneshot::Receiver<Vec<Song>> {
    let (reply, answer) = oneshot::channel();
    if let Some(history) = History::get(cx) {
        history.with_database(move |connection| {
            let found = songs::all(connection).unwrap_or_else(|error| {
                tracing::warn!("library: can't read the songs: {error}");
                Vec::new()
            });
            let _ = reply.send(found);
        });
    }
    answer
}

/// The rows of a category's `names`, each going to its songs.
pub(super) fn name_rows(route: &Route, names: &[String]) -> Vec<Row> {
    let Some((grouping, None)) = place(route) else { return Vec::new() };
    let (category, to): (MusicCategory, fn(Option<String>) -> MusicRoute) = match grouping {
        Grouping::Albums => (MusicCategory::Albums, MusicRoute::Albums),
        Grouping::Artists => (MusicCategory::Artists, MusicRoute::Artists),
        Grouping::Genres => (MusicCategory::Genres, MusicRoute::Genres),
    };
    let row = |name: &String| {
        let target = Target::Go(Route::Music(to(Some(name.clone()))));
        Row::new(name.clone(), category_icon(category), target)
    };
    names.iter().map(row).collect()
}
