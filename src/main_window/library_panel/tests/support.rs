//! What the library panel's topic tests share: a main window whose database runs on the test's
//! thread, with saved playlists and podcasts, and a way to choose a menu's entries and read the
//! toasts that follow.

use std::path::PathBuf;
use std::sync::Arc;

use gpui::{TestAppContext, VisualTestContext};
use url::Url;

use crate::history::tests::support::TestHistory;
use crate::main_window::MainWindow;
use crate::main_window::tests::{main_view, open, press_toast};
use crate::navigation::route::Route;
use crate::podcasts::feed::{Episode, Feed};
use crate::podcasts::fetch::{Failure, FeedSource};
use crate::settings;
use crate::ui::menu::MenuItem;

/// The feed address the stand-in serves.
pub(super) const FEED: &str = "https://e.com/rss";

/// Serves one show at [`FEED`], whose episodes are the files it was given; any other address
/// is missing.
struct Show(Vec<PathBuf>);

impl FeedSource for Show {
    fn fetch(&self, url: &str, _: Option<&Url>) -> Result<Feed, Failure> {
        if url != FEED {
            return Err(Failure::Status(404));
        }
        let episode = |(at, file): (usize, &PathBuf)| Episode {
            guid: at.to_string(),
            title: format!("Episode {}", at + 1),
            url: Url::from_file_path(file).expect("an absolute path").to_string(),
            published_s: Some(1_700_000_000 - at as i64 * 86_400),
            ..Episode::default()
        };
        let episodes = self.0.iter().enumerate().map(episode).collect();
        Ok(Feed { title: "The Show".into(), episodes })
    }
}

/// The main window with the library open on Home, saved playlists, favorites, Network's streams,
/// and podcasts whose feed at [`FEED`] lists `episodes` (files, so nothing goes on the network).
/// Played media opens paused, so nothing is heard.
pub(super) fn with_library(
    cx: &mut TestAppContext,
    episodes: Vec<PathBuf>,
) -> (&mut VisualTestContext, TestHistory) {
    let mut history = TestHistory::install(cx);
    let cx = open(cx);
    cx.update(|_, cx| {
        settings::update(cx, |s| s.prefs.player.start_playing = false);
        crate::playlists::init(cx);
        crate::library::favorites::init(cx);
        crate::network::init(cx);
        crate::podcasts::init_with(Arc::new(Show(episodes)), cx);
    });
    main_view(cx).update_in(cx, MainWindow::toggle_library_panel);
    history.settle(cx);
    (cx, history)
}

/// Shows `route` in the library and lets it be read.
pub(super) fn show(cx: &mut VisualTestContext, history: &mut TestHistory, route: Route) {
    main_view(cx).update(cx, |main, cx| main.navigate(route, cx));
    history.settle(cx);
}

/// Each entry's label and whether it can be chosen, separators left out.
pub(super) fn entries(items: &[MenuItem]) -> Vec<(String, bool)> {
    let entry = |item: &MenuItem| match item {
        MenuItem::Action { label, disabled, .. } | MenuItem::Check { label, disabled, .. } => {
            Some((label.to_string(), !disabled))
        }
        MenuItem::Submenu { label, items, .. } => Some((label.to_string(), !items.is_empty())),
        MenuItem::Separator => None,
    };
    items.iter().filter_map(entry).collect()
}

/// The entries' labels.
pub(super) fn labels(items: &[MenuItem]) -> Vec<String> {
    entries(items).into_iter().map(|(label, _)| label).collect()
}

/// The entry reached by following `path`'s labels through submenus.
pub(super) fn find<'a>(items: &'a [MenuItem], path: &[&str]) -> &'a MenuItem {
    let (first, rest) = path.split_first().expect("a label");
    let named = |item: &&MenuItem| match item {
        MenuItem::Action { label, .. }
        | MenuItem::Check { label, .. }
        | MenuItem::Submenu { label, .. } => label == first,
        MenuItem::Separator => false,
    };
    let item = items.iter().find(named).unwrap_or_else(|| panic!("no {first} in the menu"));
    match item {
        MenuItem::Submenu { items, .. } if !rest.is_empty() => find(items, rest),
        _ => item,
    }
}

/// Chooses the entry `path` names, as clicking it does, and lets what it started finish.
pub(super) fn choose(
    cx: &mut VisualTestContext,
    history: &mut TestHistory,
    items: &[MenuItem],
    path: &[&str],
) {
    let (MenuItem::Action { on_select, .. } | MenuItem::Check { on_select, .. }) =
        find(items, path)
    else {
        panic!("{path:?} can't be chosen");
    };
    let on_select = on_select.clone();
    cx.update(|window, cx| on_select(window, cx));
    history.settle(cx);
}

/// Presses the newest toast button labelled `label`, then runs the database work it started.
pub(super) fn press_toast_and_settle(
    cx: &mut VisualTestContext,
    history: &mut TestHistory,
    label: &str,
) {
    press_toast(cx, label);
    history.settle(cx);
}

/// Whether a dialog is open.
pub(super) fn dialog_open(cx: &mut VisualTestContext) -> bool {
    main_view(cx).read_with(cx, |main, cx| main.modals.read(cx).is_open())
}

/// The names of what the queue holds, in order.
pub(super) fn queued(cx: &mut VisualTestContext) -> Vec<String> {
    main_view(cx).read_with(cx, |main, _| {
        main.queue.entries().iter().map(|entry| entry.media.display_name()).collect()
    })
}
