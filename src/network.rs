//! Network streams (S4.11): the [`Streams`] global listing Network's categories, the operations
//! that fill them, and reading links ([`links`]).
//!
//! The rows live in the database ([`crate::db::network`]), which only the history thread
//! touches: every operation runs there through [`History::with_database`] and reads the lists
//! again, so the global is current once its task finishes and windows that observe it redraw.
//!
//! A link with a user name or password is played but never stored: the database and the
//! settings files aren't a place for passwords, and the system keychain isn't wired up yet.

pub mod links;

use futures::channel::oneshot;
use gpui::{App, Global, Task};
use rusqlite::Connection;

use crate::db::network::{self, Stream};
use crate::db::now_ms;
use crate::history::History;
use crate::navigation::route::NetworkCategory;
use crate::privacy::exclusions::Exclusions;
use crate::settings;

/// Every category's streams, as Network lists them.
#[derive(Default)]
pub struct Streams {
    streams: Vec<Stream>,
}

impl Global for Streams {}

/// Starts with no streams and reads them from the database.
pub fn init(cx: &mut App) {
    cx.set_global(Streams::default());
    run(cx, |_, _| Ok(())).detach();
}

/// `category`'s streams' URLs, in its order. Recent Streams lists none while recently played
/// items aren't saved.
pub fn urls(category: NetworkCategory, cx: &App) -> Vec<String> {
    if category == NetworkCategory::Recent && !settings::get(cx).prefs.privacy.save_recently_played
    {
        return Vec::new();
    }
    let Some(streams) = cx.try_global::<Streams>() else { return Vec::new() };
    let listed = streams.streams.iter().filter(|stream| stream.category == category);
    listed.map(|stream| stream.url.clone()).collect()
}

/// How many recent streams are kept, listed or not (Clear History counts them while history is
/// off too).
pub fn recent_count(cx: &App) -> usize {
    let Some(streams) = cx.try_global::<Streams>() else { return 0 };
    streams.streams.iter().filter(|stream| stream.category == NetworkCategory::Recent).count()
}

/// Lists `urls` first in Recent Streams, if recently played items are saved. Links with
/// credentials, and those "Never remember items matching" names, are left out.
pub fn record_recent(urls: &[String], cx: &mut App) {
    if !settings::get(cx).prefs.privacy.save_recently_played {
        return;
    }
    let exclusions = Exclusions::from_settings(cx);
    let mut urls = storable(urls);
    urls.retain(|url| !exclusions.excludes_url(url));
    if urls.is_empty() {
        return;
    }
    run(cx, move |connection, now| network::record_recent(connection, &urls, now)).detach();
}

/// Files `url` under `category`, unless it has credentials.
pub fn add(url: &str, category: NetworkCategory, cx: &mut App) {
    if links::has_credentials(url) {
        return;
    }
    let url = url.to_owned();
    run(cx, move |connection, now| network::add(connection, &url, category, now)).detach();
}

/// Takes `url` off `category`'s list.
pub fn remove(url: &str, category: NetworkCategory, cx: &mut App) {
    let url = url.to_owned();
    run(cx, move |connection, _| network::remove(connection, &url, category)).detach();
}

/// Forgets the recent streams (Clear History).
pub fn clear_recent(cx: &mut App) {
    run(cx, |connection, _| network::clear_recent(connection)).detach();
}

/// The links of `urls` that may be stored: those without a user name or password.
pub fn storable(urls: &[String]) -> Vec<String> {
    urls.iter().filter(|url| !links::has_credentials(url)).cloned().collect()
}

/// Runs `work` on the history thread's database, then reads the lists into the global.
fn run(
    cx: &mut App,
    work: impl FnOnce(&mut Connection, i64) -> rusqlite::Result<()> + Send + 'static,
) -> Task<Option<()>> {
    let Some(history) = History::get(cx) else { return Task::ready(None) };
    let (reply, answer) = oneshot::channel();
    history.with_database(move |connection| {
        let done = work(connection, now_ms())
            .and_then(|()| network::list(connection))
            .inspect_err(|error| tracing::warn!("network streams: {error}"));
        // The app may be quitting.
        let _ = reply.send(done.ok());
    });
    cx.spawn(async move |cx| {
        let streams = answer.await.ok().flatten()?;
        cx.update(|cx| cx.set_global(Streams { streams }));
        Some(())
    })
}
