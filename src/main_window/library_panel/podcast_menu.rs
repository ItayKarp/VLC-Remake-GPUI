//! The menus of Network → Podcasts (S8.17.2, not designed). A podcast's: Refresh, Mark All as
//! Played, Copy Feed Link and Unsubscribe. An episode's: Play, Add to Queue, Mark as Played or
//! Unplayed, and Copy Link. Several episodes play one after another.

use gpui::{App, ClipboardItem, Context, WeakEntity, Window};

use super::MainWindow;
use super::item_menu::{on_all, on_one};
use crate::app::media::Media;
use crate::app::request::{OpenRequest, Placement};
use crate::assets::icons;
use crate::db::podcasts::{EpisodeId, FeedId};
use crate::podcasts;
use crate::ui::menu::MenuItem;
use crate::ui::toast::{Level, Toast};

/// One episode a menu acts on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EpisodeTarget {
    /// Its row.
    pub(super) id: EpisodeId,
    /// Its file's URL.
    pub(super) url: String,
    /// Whether it was played.
    pub(super) played: bool,
}

impl EpisodeTarget {
    /// Episode `id`, while it is listed.
    pub(super) fn of(id: EpisodeId, cx: &App) -> Option<Self> {
        let episode = podcasts::episode(id, cx)?;
        Some(Self { id, url: episode.url.clone(), played: episode.played })
    }
}

/// The menu of the podcasts `feeds`.
pub(super) fn feed_items(main: &WeakEntity<MainWindow>, feeds: &[FeedId]) -> Vec<MenuItem> {
    let refresh = on_all(main, feeds, tr!("Refresh"), icons::REFRESH, |_, feeds, _, cx| {
        for id in feeds {
            refresh(*id, cx);
        }
    });
    let mark = on_all(main, feeds, tr!("Mark All as Played"), icons::CHECK, |_, feeds, _, cx| {
        let ids = feeds.iter().flat_map(|feed| podcasts::episodes(*feed, cx).map(|e| e.id));
        podcasts::set_played(ids.collect(), true, cx);
    });
    let copy = on_one(main, feeds, tr!("Copy Feed Link"), icons::LIST, |_, id, _, cx| {
        if let Some(feed) = podcasts::subscription(*id, cx) {
            cx.write_to_clipboard(ClipboardItem::new_string(feed.url.clone()));
        }
    });
    let unsubscribe = on_all(main, feeds, tr!("Unsubscribe"), icons::REMOVE, |_, feeds, _, cx| {
        for id in feeds {
            podcasts::unsubscribe(*id, cx);
        }
    });
    vec![refresh, mark, copy, MenuItem::Separator, unsubscribe]
}

/// The menu of `episodes`.
pub(super) fn episode_items(
    main: &WeakEntity<MainWindow>,
    episodes: &[EpisodeTarget],
) -> Vec<MenuItem> {
    let play = |label, icon, placement| {
        on_all(main, episodes, label, icon, move |main, episodes, window, cx| {
            play(main, episodes, placement, window, cx);
        })
    };
    let played = !episodes.is_empty() && episodes.iter().all(|episode| episode.played);
    let (label, icon) = if played {
        (tr!("Mark as Unplayed"), icons::REPLAY)
    } else {
        (tr!("Mark as Played"), icons::CHECK)
    };
    let mark = on_all(main, episodes, label, icon, move |_, episodes, _, cx| {
        podcasts::set_played(episodes.iter().map(|episode| episode.id).collect(), !played, cx);
    });
    let copy = on_one(main, episodes, tr!("Copy Link"), icons::LIST, |_, episode, _, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string(episode.url.clone()));
    });
    vec![
        play(tr!("Play"), icons::PLAY, Placement::Replace),
        play(tr!("Add to Queue"), icons::QUEUE, Placement::Enqueue),
        mark,
        copy,
    ]
}

/// Plays `episodes` as `placement` says.
fn play(
    main: &mut MainWindow,
    episodes: &[EpisodeTarget],
    placement: Placement,
    window: &mut Window,
    cx: &mut Context<MainWindow>,
) {
    let items = episodes.iter().map(|episode| Media::Url(episode.url.clone())).collect();
    main.open(OpenRequest::placed(items, placement), window, cx);
}

/// Downloads feed `id` again, saying in a toast when that fails.
fn refresh(id: FeedId, cx: &mut Context<MainWindow>) {
    let refreshing = podcasts::refresh(id, cx);
    cx.spawn(async move |main, cx| {
        let Err(failure) = refreshing.await else { return };
        let toast = Toast::new(Level::Error, failure.message());
        main.update(cx, |main, cx| main.show_toast(toast, cx)).ok();
    })
    .detach();
}
