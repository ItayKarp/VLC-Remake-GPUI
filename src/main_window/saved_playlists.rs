//! Saved playlists in the main window (S3.5): New Playlist and Save Queue as Playlist open the
//! New Playlist dialog ([`super::new_playlist`]), and the video menu's Playlist ▸ adds the current
//! item to a playlist and lists the saved ones ([`menu`]) to play, queue, rename, duplicate or
//! delete, and export to a file ([`export`]). Import Playlist… saves playlist files ([`import`]).
//!
//! The library's Playlists section lists them too, each with the playlist menu (S4.8.3); Rename,
//! Duplicate and Delete are only here until the playlist view (S3.5.3) offers them.

mod export;
mod import;
mod menu;

use gpui::{App, Context, Div, Task, WeakEntity, Window, prelude::*};

use super::MainWindow;
use super::new_playlist::NewPlaylistDialog;
use crate::app::media::Media;
use crate::app::request::{OpenRequest, Placement};
use crate::db::playlists::{PlaylistId, Summary};
use crate::keymap::actions::{ImportPlaylist, NewPlaylist, SaveQueueAsPlaylist};
use crate::playlists::{self, Deleted};
use crate::ui::modal::{Dialog, DialogButton};
use crate::ui::text_input::TextInput;
use crate::ui::toast::{Level, Toast};

impl MainWindow {
    /// Adds the handlers for New Playlist, Save Queue as Playlist and Import Playlist.
    pub(super) fn playlist_actions(root: Div, cx: &mut Context<Self>) -> Div {
        root.on_action(cx.listener(|this, _: &NewPlaylist, window, cx| {
            this.show_new_playlist(Vec::new(), window, cx);
        }))
        .on_action(cx.listener(|this, _: &SaveQueueAsPlaylist, window, cx| {
            let files = this.queue.entries().iter().map(|entry| entry.media.clone()).collect();
            this.show_new_playlist(files, window, cx);
        }))
        .on_action(cx.listener(|_, _: &ImportPlaylist, window, cx| {
            Self::import_playlists(window, cx);
        }))
    }

    /// Opens the New Playlist dialog holding `files`; Save saves it and says so.
    pub(super) fn show_new_playlist(
        &mut self,
        files: Vec<Media>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let main = cx.entity().downgrade();
        let on_save = move |draft, window: &mut Window, cx: &mut App| {
            let saved = playlists::create(&draft, cx);
            report_to(&main, saved, |title| format!("Saved the playlist “{title}”."), window, cx);
        };
        let modals = self.modals.downgrade();
        let dialog = cx.new(|cx| NewPlaylistDialog::new(files, modals, on_save, window, cx));
        self.modals.update(cx, |layer, cx| layer.push(dialog, window, cx));
    }

    /// Plays playlist `summary`'s items as `placement` says.
    fn play_playlist(
        summary: &Summary,
        placement: Placement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let items = playlists::items(summary.id, cx);
        cx.spawn_in(window, async move |this, cx| {
            let Some(items) = items.await.filter(|items| !items.is_empty()) else { return };
            this.update_in(cx, |this, window, cx| {
                this.open(OpenRequest::placed(items, placement), window, cx);
            })
            .ok();
        })
        .detach();
    }

    /// Adds `media` at the end of playlist `summary`.
    pub(super) fn add_to_playlist(
        summary: &Summary,
        media: &[Media],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let added = playlists::add(summary.id, media, cx);
        let title = summary.title.clone();
        let what = match media.len() {
            1 => String::new(),
            count => format!("{count} items "),
        };
        let done = move |()| format!("Added {what}to “{title}”.");
        Self::report_playlists(added, done, window, cx);
    }

    /// Asks for playlist `summary`'s new title, and renames it.
    fn rename_playlist(&mut self, summary: &Summary, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| {
            TextInput::new(window, cx)
                .with_text(&summary.title)
                .with_all_selected()
                .with_label("Playlist title")
                .with_max_len(playlists::title::MAX_CHARS)
        });
        let main = cx.entity().downgrade();
        let id = summary.id;
        let rename = {
            let input = input.clone();
            DialogButton::new("Rename").primary().on_click(move |window, cx| {
                let typed = input.read(cx).text().to_owned();
                let renamed = playlists::rename(id, &typed, cx);
                report_to(&main, renamed, |title| format!("Renamed to “{title}”."), window, cx);
            })
        };
        let body = move |_: &mut Window, _: &mut App| input.clone().into_any_element();
        let buttons = vec![DialogButton::new("Cancel"), rename];
        let dialog = cx.new(|cx| Dialog::new("Rename Playlist", body, buttons, cx));
        self.modals.update(cx, |layer, cx| layer.push(dialog, window, cx));
    }

    /// Copies playlist `summary` under a numbered title.
    fn duplicate_playlist(summary: &Summary, window: &mut Window, cx: &mut Context<Self>) {
        let copied = playlists::duplicate(summary.id, cx);
        Self::report_playlists(copied, |title| format!("Made a copy named “{title}”."), window, cx);
    }

    /// Asks before deleting playlist `summary`.
    fn confirm_delete_playlist(
        &mut self,
        summary: &Summary,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let main = cx.entity().downgrade();
        let id = summary.id;
        let delete = DialogButton::new("Delete").primary().on_click(move |_, cx| {
            main.update(cx, |_, cx| Self::delete_playlist(id, cx)).ok();
        });
        let body = |_: &mut Window, _: &mut App| {
            "The playlist is removed. Its files stay where they are.".into_any_element()
        };
        let buttons = vec![DialogButton::new("Cancel"), delete];
        let title = format!("Delete “{}”?", summary.title);
        let dialog = cx.new(|cx| Dialog::new(title, body, buttons, cx));
        self.modals.update(cx, |layer, cx| layer.push(dialog, window, cx));
    }

    /// Deletes playlist `id`, with Undo on the toast that says so.
    fn delete_playlist(id: PlaylistId, cx: &mut Context<Self>) {
        let deleted = playlists::delete(id, cx);
        cx.spawn(async move |this, cx| {
            let deleted = deleted.await;
            this.update(cx, |this, cx| this.show_toast(deleted_toast(deleted), cx)).ok();
        })
        .detach();
    }

    /// Once `task` is done, says what it did with `done`, or that it failed.
    fn report_playlists<T: 'static>(
        task: Task<Option<T>>,
        done: impl FnOnce(T) -> String + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.spawn_in(window, async move |this, cx| {
            let toast = match task.await {
                Some(value) => Toast::new(Level::Success, done(value)),
                None => Toast::new(Level::Error, "Couldn't save the change to your playlists."),
            };
            this.update(cx, |this, cx| this.show_toast(toast, cx)).ok();
        })
        .detach();
    }
}

/// [`MainWindow::report_playlists`] on the main window `main`.
fn report_to<T: 'static>(
    main: &WeakEntity<MainWindow>,
    task: Task<Option<T>>,
    done: impl FnOnce(T) -> String + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    main.update(cx, |_, cx| MainWindow::report_playlists(task, done, window, cx)).ok();
}

/// "Deleted “X”." with Undo, or the failure.
fn deleted_toast(deleted: Option<Deleted>) -> Toast {
    let Some(deleted) = deleted else {
        return Toast::new(Level::Error, "Couldn't delete the playlist.");
    };
    let message = format!("Deleted “{}”.", deleted.title());
    Toast::new(Level::Info, message).action("Undo", move |_, cx| {
        playlists::restore(deleted.clone(), cx).detach();
    })
}
