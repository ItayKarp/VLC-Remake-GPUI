//! What Network lists (S4.11.3, `sidebar-network`): "+ Add Network Stream", its built-in
//! categories (Recent Streams once something was opened, then Podcasts and Radio), and the
//! saved playlists made of links only (S4.11.2); in a category, its streams. Podcasts lists
//! "+ Subscribe to Podcast" and the feeds subscribed to before its streams, and a feed its
//! episodes, the latest first, played ones checked (S8.17.2).

use gpui::App;

use crate::assets::icons;
use crate::db::podcasts::FeedId;
use crate::main_window::library_panel::row::{Row, Target};
use crate::navigation::route::{NetworkCategory, Route};
use crate::network;
use crate::playlists;
use crate::podcasts;

/// The rows of Network's root, or of one of its categories.
pub(super) fn rows(category: Option<NetworkCategory>, cx: &App) -> Vec<Row> {
    match category {
        None => root(cx),
        Some(category) => {
            let feeds = (category == NetworkCategory::Podcasts).then(|| feed_rows(cx));
            let urls = network::urls(category, cx).into_iter();
            let streams = urls.map(|url| Row::stream(url, category));
            feeds.into_iter().flatten().chain(streams).collect()
        }
    }
}

/// "+ Subscribe to Podcast", then the feeds subscribed to.
fn feed_rows(cx: &App) -> Vec<Row> {
    let subscribe = Row::new("Subscribe to Podcast", icons::ADD, Target::Subscribe);
    let feeds = podcasts::subscriptions(cx).iter().map(|feed| {
        Row::new(feed.title.clone(), icons::STREAMING, Target::Go(Route::Podcast(feed.id)))
    });
    std::iter::once(subscribe).chain(feeds).collect()
}

/// Feed `id`'s episodes, the latest first; a played one is checked.
pub(super) fn episode_rows(id: FeedId, cx: &App) -> Vec<Row> {
    let episodes = podcasts::episodes(id, cx).map(|episode| {
        let icon = if episode.played { icons::CHECK } else { icons::PLAY };
        Row::new(episode.title.clone(), icon, Target::Episode(episode.id, episode.url.clone()))
    });
    episodes.collect()
}

/// "+ Add Network Stream", the categories, then the playlists of streams.
fn root(cx: &App) -> Vec<Row> {
    let add = Row::new("Add Network Stream", icons::ADD, Target::AddStream);
    let has_recent = !network::urls(NetworkCategory::Recent, cx).is_empty();
    let categories = NetworkCategory::ALL
        .into_iter()
        .filter(|category| *category != NetworkCategory::Recent || has_recent)
        .map(|category| {
            Row::new(category.label(), icons::STREAMING, Target::Go(Route::Network(Some(category))))
        });
    let playlists =
        playlists::summaries(cx).iter().filter(|summary| summary.streams_only).map(|summary| {
            let target = Target::Go(Route::Playlists(Some(summary.id)));
            Row::new(summary.title.clone(), icons::PLAYLIST, target)
        });
    std::iter::once(add).chain(categories).chain(playlists).collect()
}
