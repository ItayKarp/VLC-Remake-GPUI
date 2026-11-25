//! Headless tests of Network → Podcasts (S8.17.2): subscribing from its row, what a show's and
//! an episode's menus offer and what their entries do, and marking an episode played once it
//! plays to its end.

use std::path::PathBuf;

use gpui::{Modifiers, TestAppContext, VisualTestContext};

use super::super::listing::Body;
use super::super::podcast_menu::{self, EpisodeTarget};
use super::support::{
    FEED, FEED_THEN_GONE, choose, dialog_open, labels, queued, show, with_library,
};
use crate::db::podcasts::FeedId;
use crate::history::tests::support::TestHistory;
use crate::main_window::MainWindow;
use crate::main_window::tests::{main_view, tap, toast_messages};
use crate::navigation::route::{NetworkCategory, Route};
use crate::opening::scratch::scratch_dir;
use crate::podcasts;

const PODCASTS: Route = Route::Network(Some(NetworkCategory::Podcasts));

/// Two episode files, so playing one reads nothing from the network.
fn episode_files() -> Vec<PathBuf> {
    let dir = scratch_dir();
    let files = vec![dir.join("one.mp3"), dir.join("two.mp3")];
    for file in &files {
        std::fs::write(file, b"x").expect("file");
    }
    files
}

/// The library with the show at [`FEED`] subscribed to.
fn subscribed(cx: &mut TestAppContext) -> (&mut VisualTestContext, TestHistory, FeedId) {
    subscribed_to(cx, FEED)
}

/// The library with the show at `address` subscribed to.
fn subscribed_to<'a>(
    cx: &'a mut TestAppContext,
    address: &str,
) -> (&'a mut VisualTestContext, TestHistory, FeedId) {
    let (cx, mut history) = with_library(cx, episode_files());
    let task = cx.update(|_, cx| podcasts::subscribe(address, cx));
    history.settle(cx);
    let id = futures::FutureExt::now_or_never(task).expect("finished").expect("subscribed");
    (cx, history, id)
}

/// The listed rows' names, in order.
fn rows(cx: &mut VisualTestContext) -> Vec<String> {
    let main = main_view(cx);
    cx.update(|_, cx| {
        let main = main.read(cx);
        let Body::Items(items) = main.listing(cx) else { panic!("the place was read") };
        items.rows.iter().map(|row| row.label().to_string()).collect()
    })
}

/// Clicks the row drawn as `id`.
fn click(cx: &mut VisualTestContext, id: &'static str) {
    let at = cx.debug_bounds(id).expect("the row is drawn").center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
}

/// The open item menu's labels.
fn open_menu_labels(cx: &mut VisualTestContext) -> Vec<String> {
    let main = main_view(cx);
    cx.update(|_, cx| {
        let main = main.read(cx);
        let (menu, _, _) = main.library_panel.item_menu.as_ref().expect("a menu is open");
        menu.read(cx).entries().into_iter().map(|(label, _)| label).collect()
    })
}

/// Right-clicks the row drawn as `id`.
fn right_click(cx: &mut VisualTestContext, id: &'static str) {
    let at = cx.debug_bounds(id).expect("the row is drawn").center();
    cx.simulate_mouse_down(at, gpui::MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(at, gpui::MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
}

/// Types `address` into the open Subscribe to Podcast dialog and presses Subscribe.
fn subscribe_by_hand(cx: &mut VisualTestContext, history: &mut TestHistory, address: &str) {
    click(cx, "library-row-0");
    assert!(dialog_open(cx), "the row asks for the feed");
    cx.simulate_input(address);
    tap(cx, "enter");
    history.settle(cx);
}

fn route(cx: &mut VisualTestContext) -> Route {
    let main = main_view(cx);
    cx.update(|_, cx| main.read(cx).library_panel.history.current().clone())
}

#[gpui::test]
fn subscribing_from_the_row_shows_the_new_show(cx: &mut TestAppContext) {
    let (cx, mut history) = with_library(cx, episode_files());
    show(cx, &mut history, PODCASTS);
    assert_eq!(rows(cx), ["Subscribe to Podcast"]);
    subscribe_by_hand(cx, &mut history, "feed://e.com/rss");
    let Route::Podcast(id) = route(cx) else { panic!("the show's episodes are shown") };
    assert_eq!(rows(cx), ["Episode 1", "Episode 2"]);
    show(cx, &mut history, PODCASTS);
    assert_eq!(rows(cx), ["Subscribe to Podcast", "The Show"]);
    cx.update(|_, cx| assert!(podcasts::subscription(id, cx).is_some()));
}

#[gpui::test]
fn a_feed_that_cant_be_read_says_why(cx: &mut TestAppContext) {
    let (cx, mut history) = with_library(cx, episode_files());
    show(cx, &mut history, PODCASTS);
    subscribe_by_hand(cx, &mut history, "https://e.com/missing");
    assert_eq!(route(cx), PODCASTS, "nothing was subscribed to");
    let failed = toast_messages(cx);
    assert_eq!(failed.len(), 1, "only the failure is left: {failed:?}");
}

#[gpui::test]
fn a_shows_menu_copies_marks_and_unsubscribes(cx: &mut TestAppContext) {
    let (cx, mut history, id) = subscribed(cx);
    show(cx, &mut history, PODCASTS);
    right_click(cx, "library-row-1");
    let expected = ["Refresh", "Mark All as Played", "Copy Feed Link", "Unsubscribe"];
    assert_eq!(open_menu_labels(cx), expected);

    let items = podcast_menu::feed_items(&main_view(cx).downgrade(), &[id]);
    choose(cx, &mut history, &items, &["Copy Feed Link"]);
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(copied.as_deref(), Some(FEED));

    choose(cx, &mut history, &items, &["Refresh"]);
    choose(cx, &mut history, &items, &["Mark All as Played"]);
    let played = cx.update(|_, cx| podcasts::episodes(id, cx).all(|episode| episode.played));
    assert!(played);

    choose(cx, &mut history, &items, &["Unsubscribe"]);
    assert_eq!(rows(cx), ["Subscribe to Podcast"]);
    cx.write_to_clipboard(gpui::ClipboardItem::new_string("kept".into()));
    choose(cx, &mut history, &items, &["Copy Feed Link"]);
    let kept = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(kept.as_deref(), Some("kept"), "a show no longer subscribed copies nothing");
}

#[gpui::test]
fn refreshing_a_show_that_is_gone_says_why(cx: &mut TestAppContext) {
    let (cx, mut history, id) = subscribed_to(cx, FEED_THEN_GONE);
    show(cx, &mut history, PODCASTS);
    let items = podcast_menu::feed_items(&main_view(cx).downgrade(), &[id]);
    choose(cx, &mut history, &items, &["Refresh"]);
    assert_eq!(toast_messages(cx), ["Nothing was found at that address."]);
    assert_eq!(rows(cx), ["Subscribe to Podcast", "The Show"], "the show stays subscribed");
}

/// The menu of the episodes listed at `indices` of show `id`.
fn episode_menu(cx: &mut VisualTestContext, id: FeedId, indices: &[usize]) -> Vec<String> {
    let main = main_view(cx);
    let items = cx.update(|_, cx| {
        let ids: Vec<_> = podcasts::episodes(id, cx).map(|episode| episode.id).collect();
        let targets: Vec<_> =
            indices.iter().filter_map(|at| EpisodeTarget::of(ids[*at], cx)).collect();
        podcast_menu::episode_items(&main.downgrade(), &targets)
    });
    labels(&items)
}

#[gpui::test]
fn an_episodes_menu_marks_it_played_and_copies_its_link(cx: &mut TestAppContext) {
    let (cx, mut history, id) = subscribed(cx);
    show(cx, &mut history, Route::Podcast(id));
    right_click(cx, "library-row-0");
    assert_eq!(open_menu_labels(cx), ["Play", "Add to Queue", "Mark as Played", "Copy Link"]);

    let main = main_view(cx);
    let first = cx.update(|_, cx| {
        let episode = podcasts::episodes(id, cx).next().expect("listed");
        EpisodeTarget::of(episode.id, cx).expect("listed")
    });
    let items = podcast_menu::episode_items(&main.downgrade(), std::slice::from_ref(&first));
    choose(cx, &mut history, &items, &["Copy Link"]);
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(copied, Some(first.url));

    choose(cx, &mut history, &items, &["Mark as Played"]);
    assert_eq!(episode_menu(cx, id, &[0])[2], "Mark as Unplayed");
    assert_eq!(episode_menu(cx, id, &[0, 1])[2], "Mark as Played", "one of them isn't played");
}

#[gpui::test]
fn episodes_queue_one_after_another(cx: &mut TestAppContext) {
    let (cx, mut history, id) = subscribed(cx);
    show(cx, &mut history, Route::Podcast(id));
    let main = main_view(cx);
    let targets: Vec<EpisodeTarget> = cx.update(|_, cx| {
        let ids = podcasts::episodes(id, cx).map(|episode| episode.id);
        ids.filter_map(|id| EpisodeTarget::of(id, cx)).collect()
    });
    let items = podcast_menu::episode_items(&main.downgrade(), &targets);
    choose(cx, &mut history, &items, &["Add to Queue"]);
    assert_eq!(queued(cx).len(), 2);
    choose(cx, &mut history, &items, &["Play"]);
    assert_eq!(queued(cx).len(), 2, "Play replaces the queue");
}

#[gpui::test]
fn an_episode_played_to_its_end_is_marked_played(cx: &mut TestAppContext) {
    let (cx, mut history, id) = subscribed(cx);
    show(cx, &mut history, Route::Podcast(id));
    click(cx, "library-row-1");
    history.settle(cx);
    let main = main_view(cx);
    main.update(cx, MainWindow::episode_played);
    history.settle(cx);
    let played: Vec<bool> =
        cx.update(|_, cx| podcasts::episodes(id, cx).map(|episode| episode.played).collect());
    assert_eq!(played, [false, true]);
}
