//! A saved playlist row's menu (S4.8.3; `library/menus/playlist_context_menu.png`): Play,
//! Favorite, Shuffle (plays it shuffled), Export Playlist (S3.6), Add to Queue, Add Subtitle
//! Tracks (the Add Subtitles dialog To This Playlist, S6.4) and Properties & Metadata.
//!
//! Several playlists play one after another. The design's Add Audio Tracks waits for a design
//! of which audio goes with which item; the menu leaves it out until then.

use std::time::Duration;

use gpui::{App, Context, SharedString, Task, WeakEntity, Window};

use super::MainWindow;
use super::item_menu::{on_all, on_one};
use crate::app::media::Media;
use crate::app::request::{OpenRequest, Placement};
use crate::assets::icons;
use crate::db::activity::Favorite;
use crate::db::playlists::Summary;
use crate::library::favorites;
use crate::main_window::add_subtitles::Target;
use crate::main_window::control_bar::clock;
use crate::playlists;
use crate::queue;
use crate::ui::menu::MenuItem;

/// How the menu plays playlists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Playing {
    /// Their items, placed in the queue as given.
    Placed(Placement),
    /// Their items in place of the queue, with Shuffle on from a random one.
    Shuffled,
}

/// The playlist menu's items for `playlists`.
pub(super) fn items(
    main: &WeakEntity<MainWindow>,
    playlists: &[Summary],
    cx: &App,
) -> Vec<MenuItem> {
    let empty = playlists.iter().all(|summary| summary.len == 0);
    let play = |label, icon, playing| {
        on_all(main, playlists, label, icon, move |_, playlists, window, cx| {
            MainWindow::play_playlists(playlists, playing, window, cx);
        })
        .disabled(empty)
        .why_disabled("Nothing to play")
    };
    vec![
        play("Play", icons::PLAY, Playing::Placed(Placement::Replace)),
        favorite(playlists, cx),
        play("Shuffle", icons::SHUFFLE, Playing::Shuffled),
        on_one(main, playlists, "Export Playlist", icons::SAVE, |main, summary, window, cx| {
            main.show_export_playlist(summary, window, cx);
        }),
        play("Add to Queue", icons::QUEUE, Playing::Placed(Placement::Enqueue)),
        on_one(
            main,
            playlists,
            "Add Subtitle Tracks",
            icons::SUBTITLES,
            |main, summary, window, cx| {
                let target =
                    Target::Playlist { id: summary.id, title: summary.title.clone().into() };
                main.show_add_subtitles(target, window, cx);
            },
        )
        .disabled(empty)
        .why_disabled("Nothing to add subtitles to"),
        on_one(main, playlists, "Properties & Metadata", icons::INFO, |_, summary, window, cx| {
            MainWindow::show_playlist_properties(summary, window, cx);
        }),
    ]
}

/// Favorite, checked while every one of `playlists` is one.
fn favorite(playlists: &[Summary], cx: &App) -> MenuItem {
    let marked: Vec<Favorite> = playlists.iter().map(|s| Favorite::Playlist(s.id)).collect();
    let all = favorites::all(&marked, cx);
    MenuItem::check("Favorite", all, move |_, cx| favorites::mark(marked.clone(), !all, cx))
        .icon(icons::FAVORITE)
}

impl MainWindow {
    /// Plays the items of `playlists`, one playlist after another, as `playing` says.
    fn play_playlists(
        playlists: &[Summary],
        playing: Playing,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let reads: Vec<Task<Option<Vec<Media>>>> =
            playlists.iter().map(|summary| playlists::items(summary.id, cx)).collect();
        cx.spawn_in(window, async move |this, cx| {
            let mut media = Vec::new();
            for read in reads {
                media.extend(read.await.unwrap_or_default());
            }
            if media.is_empty() {
                return;
            }
            this.update_in(cx, |this, window, cx| match playing {
                Playing::Placed(placement) => {
                    this.open(OpenRequest::placed(media, placement), window, cx);
                }
                Playing::Shuffled => {
                    this.queue.set_shuffle(true);
                    let start = queue::random_start(media.len());
                    this.play_cards(media, start, window, cx);
                    this.show_osd("Shuffle: On", cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// Properties & Metadata of playlist `summary`: its title, how many items it has, and
    /// their length.
    fn show_playlist_properties(summary: &Summary, window: &mut Window, cx: &mut Context<Self>) {
        let read = playlists::export_items(summary.id, cx);
        let title: SharedString = summary.title.clone().into();
        cx.spawn_in(window, async move |this, cx| {
            let items = read.await.unwrap_or_default();
            let durations: Vec<_> = items.iter().map(|item| item.duration).collect();
            let rows = vec![
                ("Title", title),
                ("Items", items.len().to_string().into()),
                ("Length", length_text(&durations).into()),
            ];
            this.update_in(cx, |this, window, cx| {
                this.show_info_dialog("Properties & Metadata", rows, window, cx);
            })
            .ok();
        })
        .detach();
    }
}

/// The items' total length; "at least" it while some lengths are unknown, "Unknown" when none
/// is known.
fn length_text(durations: &[Option<Duration>]) -> String {
    let known: Duration = durations.iter().flatten().sum();
    let missing = durations.iter().any(Option::is_none);
    match (known.is_zero() && missing, missing) {
        (true, _) => "Unknown".to_owned(),
        (false, true) => format!("At least {}", clock::format(known)),
        (false, false) => clock::format(known),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[expect(clippy::unnecessary_wraps, reason = "a known length, as the playlist reads it")]
    const fn secs(seconds: u64) -> Option<Duration> {
        Some(Duration::from_secs(seconds))
    }

    #[test]
    fn the_length_says_when_it_is_partial() {
        assert_eq!(length_text(&[secs(60), secs(30)]), "00:01:30");
        assert_eq!(length_text(&[secs(60), None]), "At least 00:01:00");
        assert_eq!(length_text(&[None, None]), "Unknown");
        assert_eq!(length_text(&[]), "00:00:00");
    }
}
