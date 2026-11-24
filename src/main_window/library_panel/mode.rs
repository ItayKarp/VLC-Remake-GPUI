//! How the library shows (S4.10.1, S4.10.2): the title bar's sidebar toggle and L show and hide
//! the sidebar over the player; the panel's pop-out switches to full library mode, where the
//! miniplayer's pop-out, the toggle and L go back to the player. The folder chip opens the
//! sidebar at the playing file's folder (S2.1.3).
//!
//! The sidebar shows between the bars and hides with the controls, so it is never left over a
//! video whose controls hid; the pointer over it keeps them up.
//!
//! Full library mode keeps the route the sidebar was at, and from then on remembers its own
//! (S4.5.6).
//!
//! With "Use the media library" off (S5.9.4) the library never shows: turning it off hides it and
//! offers, in a toast, to delete what was indexed, which is otherwise kept for later.

use std::path::Path;

use gpui::{Context, Div, Stateful, Window};

use super::MainWindow;
use super::file_menu::source_route;
use crate::db::metadata::MediaKind;
use crate::library;
use crate::main_window::TITLE_BAR_HEIGHT;
use crate::main_window::auto_hide::rules::Chrome;
use crate::navigation::Presentation;
use crate::navigation::route::Route;
use crate::settings::{self, SettingsStore};
use crate::theme::Palette;
use crate::ui::toast::{Level, Toast};

impl MainWindow {
    /// The library while it shows: full library mode under the title bar, or the sidebar over
    /// the player between the bars that show, hidden with the controls (S4.10.1).
    pub(in crate::main_window) fn render_library(
        &self,
        p: &Palette,
        chrome: Chrome,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        if !library::is_enabled(cx) {
            return None;
        }
        let top = if chrome.title_bar { TITLE_BAR_HEIGHT } else { 0. };
        let bottom = if chrome.controls { self.control_bar_height() } else { 0. };
        let insets = match self.library_panel.shown? {
            Presentation::Full => (top, 0.),
            Presentation::Overlay if chrome.controls => (top, bottom),
            // Never a panel stuck over a video whose controls hid.
            Presentation::Overlay => return None,
        };
        self.render_library_panel(p, insets, cx)
    }

    /// Shows or hides the sidebar (the title bar toggle and L); in full library mode, goes back
    /// to the player.
    pub(in crate::main_window) fn toggle_library_panel(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let shown = match self.library_panel.shown {
            None => Some(Presentation::Overlay),
            Some(_) => None,
        };
        self.show_library(shown, window, cx);
    }

    /// Switches to full library mode (the panel's pop-out).
    pub(super) fn open_full_library(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_popovers(window, cx);
        self.close_queue_panel(window, cx);
        self.show_library(Some(Presentation::Full), window, cx);
        let route = self.library_panel.history.current().clone();
        settings::update(cx, |s| Presentation::Full.remember(&route, &mut s.local.library_routes));
    }

    /// Leaves full library mode for the player, the library hidden (the miniplayer's pop-out).
    pub(in crate::main_window) fn leave_full_library(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.library_panel.is_full() {
            self.show_library(None, window, cx);
        }
    }

    /// Opens the library at `folder`, which holds media of `kind` (the folder chip): in Songs
    /// for audio inside a music folder, else in Videos, "Not in library" outside every media
    /// folder.
    pub(in crate::main_window) fn show_library_folder(
        &mut self,
        folder: &Path,
        kind: MediaKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let local = &settings::get(cx).local;
        let roots = (local.media_folders.as_slice(), local.music_folders.as_slice());
        let route = source_route(folder, kind, roots)
            .unwrap_or_else(|| Route::Videos(Some(folder.to_path_buf())));
        self.show_library_route(route, window, cx);
    }

    /// Opens the library at `route`: in the sidebar unless it already shows.
    pub(in crate::main_window) fn show_library_route(
        &mut self,
        route: Route,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.library_panel.is_open() {
            self.show_library(Some(Presentation::Overlay), window, cx);
        }
        self.navigate(route, cx);
    }

    /// Shows the library as `shown`, or hides it, closing its menus and search, and gives the
    /// listing the keyboard when it shows.
    fn show_library(
        &mut self,
        shown: Option<Presentation>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if shown.is_some() && !library::is_enabled(cx) {
            return;
        }
        let panel = &mut self.library_panel;
        panel.shown = shown;
        panel.close_menus();
        panel.search = None;
        if shown.is_some() {
            self.reload_folder(cx);
            let handle = self.library_panel.list_focus.clone();
            window.defer(cx, move |window, cx| handle.focus(window, cx));
        } else if self.library_panel.focus.contains_focused(window, cx) {
            self.focus_handle.focus(window, cx);
        }
        cx.notify();
    }

    /// Hides the library when "Use the media library" is turned off, and offers to delete what
    /// it indexed (S5.9.4).
    pub(in crate::main_window) fn follow_library_switch(
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut was_enabled = library::is_enabled(cx);
        cx.observe_global_in::<SettingsStore>(window, move |this, window, cx| {
            let enabled = library::is_enabled(cx);
            if std::mem::replace(&mut was_enabled, enabled) && !enabled {
                this.show_library(None, window, cx);
                let toast =
                    Toast::new(Level::Info, tr!("The library is off. Keep what it indexed?"))
                        .action(tr!("Delete Index"), |_, cx| library::delete_index(cx));
                this.show_toast(toast, cx);
            }
        })
        .detach();
    }
}
