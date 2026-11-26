//! Podcasts (S8.17.2): the feeds subscribed to in Network → Podcasts and their episodes, in the
//! [`Podcasts`] global; subscribing, refreshing and unsubscribing; and marking episodes played,
//! by hand or when one plays to its end.
//!
//! Feeds are downloaded ([`fetch`]) and read ([`feed`]) off the UI thread; the rows live in the
//! database ([`crate::db::podcasts`]), which only the history thread touches, so every change
//! runs there and reads the lists again, like Network's streams. Every feed is refreshed once
//! when the app starts.
//!
//! Feeds come from the [`FeedSource`] [`init_with`] was given: the web in the app, a stand-in in
//! tests.

pub mod dates;
pub mod feed;
pub mod fetch;

use std::sync::Arc;

use futures::channel::oneshot;
use gpui::{App, AsyncApp, Global, Task};
use rusqlite::Connection;

use self::feed::Feed;
use self::fetch::{FeedSource, Web};
use crate::db::now_ms;
use crate::db::podcasts::{self, Episode, EpisodeId, FeedId, Subscription};
use crate::history::History;
use crate::settings;

/// The feeds and their episodes, as Network → Podcasts lists them.
#[derive(Default)]
pub struct Podcasts {
    subscriptions: Vec<Subscription>,
    episodes: Vec<Episode>,
}

impl Global for Podcasts {}

/// Where feeds are downloaded from.
struct Source(Arc<dyn FeedSource>);

impl Global for Source {}

/// Reads the subscriptions from the database, then refreshes every feed from the web.
pub fn init(cx: &mut App) {
    init_with(Arc::new(Web), cx);
}

/// Reads the subscriptions from the database, then refreshes every feed from `source`.
pub(crate) fn init_with(source: Arc<dyn FeedSource>, cx: &mut App) {
    cx.set_global(Source(source));
    cx.set_global(Podcasts::default());
    let read = run(cx, |_, _| Ok(()));
    cx.spawn(async move |cx| {
        read.await;
        let ids = cx.update(|cx| subscriptions(cx).iter().map(|feed| feed.id).collect::<Vec<_>>());
        for id in ids {
            if let Err(failure) = refresh_now(id, cx).await {
                tracing::info!("podcasts: a feed didn't refresh: {}", failure.message());
            }
        }
    })
    .detach();
}

/// The feeds subscribed to, by title.
pub fn subscriptions(cx: &App) -> &[Subscription] {
    cx.try_global::<Podcasts>().map_or(&[], |podcasts| &podcasts.subscriptions)
}

/// The feed `id`, if subscribed.
pub fn subscription(id: FeedId, cx: &App) -> Option<&Subscription> {
    subscriptions(cx).iter().find(|feed| feed.id == id)
}

/// Feed `id`'s episodes, the latest first.
pub fn episodes(id: FeedId, cx: &App) -> impl Iterator<Item = &Episode> {
    let all = cx.try_global::<Podcasts>().map_or(&[][..], |podcasts| &podcasts.episodes);
    all.iter().filter(move |episode| episode.feed == id)
}

/// The episode `id`, if listed.
pub fn episode(id: EpisodeId, cx: &App) -> Option<&Episode> {
    let podcasts = cx.try_global::<Podcasts>()?;
    podcasts.episodes.iter().find(|episode| episode.id == id)
}

/// Subscribes to the feed `typed` names: downloads it, then lists it. Answers its id, or why it
/// couldn't be read.
pub fn subscribe(typed: &str, cx: &mut App) -> Task<Result<FeedId, fetch::Failure>> {
    let url = match fetch::address(typed) {
        Ok(url) => url,
        Err(failure) => return Task::ready(Err(failure)),
    };
    let download = download(url.clone(), cx);
    cx.spawn(async move |cx| {
        let feed = download.await?;
        let (reply, answer) = oneshot::channel();
        let stored = cx.update(|cx| {
            run(cx, move |connection, now| {
                let id = podcasts::subscribe(connection, &url, &feed, now)?;
                let _ = reply.send(id);
                Ok(())
            })
        });
        stored.await;
        answer.await.map_err(|_| fetch::Failure::Network(tr!("the library isn't available").into()))
    })
}

/// Downloads feed `id` again and takes in its new episodes.
pub fn refresh(id: FeedId, cx: &mut App) -> Task<Result<(), fetch::Failure>> {
    cx.spawn(async move |cx| refresh_now(id, cx).await)
}

/// Unsubscribes from feed `id`.
pub fn unsubscribe(id: FeedId, cx: &mut App) {
    run(cx, move |connection, _| podcasts::unsubscribe(connection, id)).detach();
}

/// Marks episodes `ids` played or not.
pub fn set_played(ids: Vec<EpisodeId>, played: bool, cx: &mut App) {
    run(cx, move |connection, _| podcasts::set_played(connection, &ids, played)).detach();
}

/// An episode whose file is `url` played to its end: it is marked played.
pub fn played_to_end(url: &str, cx: &mut App) {
    let listed = cx.try_global::<Podcasts>().is_some_and(|podcasts| {
        podcasts.episodes.iter().any(|episode| episode.url == url && !episode.played)
    });
    if listed {
        let url = url.to_owned();
        run(cx, move |connection, _| podcasts::played_url(connection, &url).map(|_| ())).detach();
    }
}

async fn refresh_now(id: FeedId, cx: &mut AsyncApp) -> Result<(), fetch::Failure> {
    let url = cx.update(|cx| subscription(id, cx).map(|feed| feed.url.clone()));
    let Some(url) = url else { return Ok(()) };
    let feed = cx.update(|cx| download(url, cx)).await?;
    let stored = cx.update(|cx| {
        run(cx, move |connection, now| podcasts::refresh(connection, id, &feed, now).map(|_| ()))
    });
    stored.await;
    Ok(())
}

/// Downloads and reads the feed at `url`, off the UI thread, from the source [`init_with`] was
/// given (the web before it runs).
fn download(url: String, cx: &App) -> Task<Result<Feed, fetch::Failure>> {
    let proxy = settings::proxy_url(&settings::get(cx).prefs.network.http_proxy);
    let source = cx.try_global::<Source>().map_or_else(|| Arc::new(Web) as _, |s| s.0.clone());
    cx.background_executor().spawn(async move { source.fetch(&url, proxy.as_ref()) })
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
            .and_then(|()| {
                Ok((podcasts::subscriptions(connection)?, podcasts::episodes(connection)?))
            })
            .inspect_err(|error| tracing::warn!("podcasts: {error}"));
        // The app may be quitting.
        let _ = reply.send(done.ok());
    });
    cx.spawn(async move |cx| {
        let (subscriptions, episodes) = answer.await.ok().flatten()?;
        cx.update(|cx| cx.set_global(Podcasts { subscriptions, episodes }));
        Some(())
    })
}

#[cfg(test)]
mod tests;
