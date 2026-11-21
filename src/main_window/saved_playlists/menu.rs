//! The saved-playlist items of the video menu's Playlist ▸: New Playlist…, Save Queue as
//! Playlist…, Import Playlist…, Add to Playlist ▸ (the current item, to a saved playlist or a new
//! one) and Saved Playlists ▸, where each playlist has Play, Add to Queue, Rename…, Duplicate,
//! Export… and Delete….

use gpui::{App, WeakEntity};

use super::super::MainWindow;
use crate::app::media::Media;
use crate::app::request::Placement;
use crate::assets::icons;
use crate::db::playlists::Summary;
use crate::keymap::actions as act;
use crate::main_window::main_menu;
use crate::playlists;
use crate::ui::menu::MenuItem;

impl MainWindow {
    /// The saved-playlist items, for Playlist ▸.
    pub(in crate::main_window) fn saved_playlist_items(
        &self,
        main: &WeakEntity<Self>,
        cx: &App,
    ) -> Vec<MenuItem> {
        let save_queue =
            main_menu::dispatching(&act::SaveQueueAsPlaylist, "Save Queue as Playlist…", cx)
                .disabled(self.queue.len() == 0)
                .why_disabled("The queue is empty");
        vec![
            main_menu::dispatching(&act::NewPlaylist, "New Playlist…", cx).icon(icons::PLAYLIST),
            save_queue,
            main_menu::dispatching(&act::ImportPlaylist, "Import Playlist…", cx),
            self.add_to_playlist_menu(main, cx),
            saved_playlists_menu(main, cx),
        ]
    }

    /// Add to Playlist ▸: the saved playlists, then New Playlist… holding the current item.
    fn add_to_playlist_menu(&self, main: &WeakEntity<Self>, cx: &App) -> MenuItem {
        let media = self.player_media(cx);
        let Some(media) = media else {
            return MenuItem::submenu("Add to Playlist", Vec::new())
                .why_disabled("No media playing");
        };
        let mut items: Vec<MenuItem> = playlists::summaries(cx)
            .iter()
            .map(|summary| add_item(main, summary, &media))
            .collect();
        if !items.is_empty() {
            items.push(MenuItem::Separator);
        }
        let main = main.clone();
        items.push(MenuItem::action("New Playlist…", move |window, cx| {
            let files = vec![media.clone()];
            main.update(cx, |main, cx| main.show_new_playlist(files, window, cx)).ok();
        }));
        MenuItem::submenu("Add to Playlist", items)
    }
}

/// An item that adds `media` to playlist `summary`.
fn add_item(main: &WeakEntity<MainWindow>, summary: &Summary, media: &Media) -> MenuItem {
    let (main, summary, media) = (main.clone(), summary.clone(), media.clone());
    MenuItem::action(summary.title.clone(), move |window, cx| {
        let media = media.clone();
        main.update(cx, |_, cx| MainWindow::add_to_playlist(&summary, media, window, cx)).ok();
    })
}

/// Saved Playlists ▸: each playlist and what can be done with it.
fn saved_playlists_menu(main: &WeakEntity<MainWindow>, cx: &App) -> MenuItem {
    let items = playlists::summaries(cx).iter().map(|summary| playlist_menu(main, summary));
    MenuItem::submenu("Saved Playlists", items.collect())
        .icon(icons::PLAYLIST)
        .why_disabled("No saved playlists yet")
}

/// One playlist's submenu.
fn playlist_menu(main: &WeakEntity<MainWindow>, summary: &Summary) -> MenuItem {
    let empty = summary.len == 0;
    let items = vec![
        on_playlist(main, summary, "Play", |_, summary, window, cx| {
            MainWindow::play_playlist(summary, Placement::Replace, window, cx);
        })
        .icon(icons::PLAY)
        .disabled(empty)
        .why_disabled("This playlist is empty"),
        on_playlist(main, summary, "Add to Queue", |_, summary, window, cx| {
            MainWindow::play_playlist(summary, Placement::Enqueue, window, cx);
        })
        .icon(icons::QUEUE)
        .disabled(empty)
        .why_disabled("This playlist is empty"),
        MenuItem::Separator,
        on_playlist(main, summary, "Rename…", MainWindow::rename_playlist).icon(icons::EDIT),
        on_playlist(main, summary, "Duplicate", |_, summary, window, cx| {
            MainWindow::duplicate_playlist(summary, window, cx);
        }),
        on_playlist(main, summary, "Export…", MainWindow::show_export_playlist),
        on_playlist(main, summary, "Delete…", MainWindow::confirm_delete_playlist)
            .icon(icons::REMOVE),
    ];
    let label = match summary.len {
        1 => format!("{} (1 item)", summary.title),
        n => format!("{} ({n} items)", summary.title),
    };
    MenuItem::submenu(label, items).icon(icons::PLAYLIST)
}

/// An item that runs `f` with playlist `summary` on `main`.
fn on_playlist(
    main: &WeakEntity<MainWindow>,
    summary: &Summary,
    label: &'static str,
    f: impl Fn(&mut MainWindow, &Summary, &mut gpui::Window, &mut gpui::Context<MainWindow>) + 'static,
) -> MenuItem {
    let (main, summary) = (main.clone(), summary.clone());
    MenuItem::action(label, move |window, cx| {
        main.update(cx, |main, cx| f(main, &summary, window, cx)).ok();
    })
}
