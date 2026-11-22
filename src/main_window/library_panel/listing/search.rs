//! Search results in the listing (S4.7.5): the media a search route's query finds in its scope,
//! as cards, read from the index in the background.

use futures::channel::oneshot;
use gpui::{App, Context, Task};

use super::{Contents, FolderState};
use crate::db::metadata::MediaKind;
use crate::history::History;
use crate::history::policy::WatchedRule;
use crate::library::folder_view::Card;
use crate::library::search::{self, Query};
use crate::main_window::MainWindow;
use crate::navigation::route::{Route, Section};
use crate::settings;

/// Reads what a search route finds, in the background; `None` for other routes.
pub(super) fn read(route: Route, cx: &mut Context<MainWindow>) -> Option<Task<()>> {
    let Route::Search(search) = &route else { return None };
    let wanted = match search.scope {
        Section::Videos => Some(MediaKind::Video),
        Section::Music => Some(MediaKind::Audio),
        _ => None,
    };
    let query = Query::parse(&search.query);
    let rule = WatchedRule::from_settings(&settings::get(cx).prefs.player);
    let found = query.map(|query| find(query, wanted, rule, cx));
    Some(cx.spawn(async move |this, cx| {
        let cards = match found {
            Some(found) => found.await.unwrap_or_default(),
            None => Vec::new(),
        };
        let state = FolderState::Read(Contents { cards, ..Contents::default() });
        this.update(cx, |this, cx| this.read_into(&route, state, cx)).ok();
    }))
}

/// The cards `query` finds, searched on the history thread.
fn find(
    query: Query,
    wanted: Option<MediaKind>,
    rule: WatchedRule,
    cx: &App,
) -> oneshot::Receiver<Vec<Card>> {
    let (reply, answer) = oneshot::channel();
    if let Some(history) = History::get(cx) {
        history.with_database(move |connection| {
            let found = search::find(connection, &query, wanted, rule).unwrap_or_else(|error| {
                tracing::warn!("library: can't search: {error}");
                Vec::new()
            });
            let _ = reply.send(found);
        });
    }
    answer
}
