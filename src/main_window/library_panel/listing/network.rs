//! What Network lists (S4.11.3, `sidebar-network`): "+ Add Network Stream", its built-in
//! categories (Recent Streams once something was opened, then Podcasts and Radio), and the
//! saved playlists made of links only (S4.11.2); in a category, its streams.

use gpui::App;

use crate::assets::icons;
use crate::main_window::library_panel::row::{Row, Target};
use crate::navigation::route::{NetworkCategory, Route};
use crate::network;
use crate::playlists;

/// The rows of Network's root, or of one of its categories.
pub(super) fn rows(category: Option<NetworkCategory>, cx: &App) -> Vec<Row> {
    match category {
        None => root(cx),
        Some(category) => {
            let urls = network::urls(category, cx).into_iter();
            urls.map(|url| Row::stream(url, category)).collect()
        }
    }
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
