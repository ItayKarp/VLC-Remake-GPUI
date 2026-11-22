//! Open Network Stream in the main window: the menu command (Ctrl+N), the rail's "+" menu and
//! Network's "+ Add Network Stream" open the dialog, and its Save plays the links or saves them
//! as a playlist (S4.11.2).

use gpui::{App, AppContext as _, Context, Div, WeakEntity, Window, prelude::*};

use super::{NetworkStreamDialog, Saved};
use crate::app::media::Media;
use crate::app::request::OpenRequest;
use crate::keymap::actions::OpenNetworkStream;
use crate::main_window::MainWindow;
use crate::network;
use crate::playlists::{self, Draft};

impl MainWindow {
    /// Adds the handler for Open Network Stream.
    pub(in crate::main_window) fn network_actions(root: Div, cx: &mut Context<Self>) -> Div {
        root.on_action(cx.listener(|this, _: &OpenNetworkStream, window, cx| {
            this.show_network_stream(window, cx);
        }))
    }

    /// Opens the Open Network Stream dialog.
    pub(in crate::main_window) fn show_network_stream(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let main = cx.entity().downgrade();
        let on_save = move |saved, window: &mut Window, cx: &mut App| {
            act_on(&main, saved, window, cx);
        };
        let dialog = cx.new(|cx| NetworkStreamDialog::new(on_save, window, cx));
        self.modals.update(cx, |layer, cx| layer.push(dialog, window, cx));
    }
}

/// Plays the links and lists them in Recent Streams, or saves them as a playlist.
fn act_on(main: &WeakEntity<MainWindow>, saved: Saved, window: &mut Window, cx: &mut App) {
    match saved {
        Saved::Play(urls) => {
            network::record_recent(&urls, cx);
            let items = urls.into_iter().map(Media::Url).collect();
            main.update(cx, |main, cx| main.open(OpenRequest::replace(items), window, cx)).ok();
        }
        Saved::Playlist { title, urls } => {
            let items = urls.into_iter().map(Media::Url).collect();
            let draft = Draft { title, thumbnail: None, items };
            let created = playlists::create(&draft, cx);
            let done = |title| format!("Saved the playlist “{title}”.");
            main.update(cx, |_, cx| MainWindow::report_playlists(created, done, window, cx)).ok();
        }
    }
}
