//! Settings → Library's Media Folders list (S5.9.3, not designed; D11 puts it at the top of the
//! page): each folder the library indexes, with whether it holds videos or music, its status and
//! scan progress (S4.2.6), **Rescan** and **Remove**, then buttons that add a folder.
//!
//! Adding asks for a folder, then places it by its canonical path ([`folders::placement`]): the
//! same folder twice or one inside a media folder is refused with the reason under the list, and
//! a folder holding media folders takes their place. A whole drive or the home folder asks
//! first, unless confirmations are off (S5.3.3). Removing the last folder leaves the library
//! empty, which shows its empty state.

mod add;

use std::path::{Path, PathBuf};

use gpui::{
    ClickEvent, Context, Div, IntoElement, Render, SharedString, Subscription, Window, div,
    prelude::*, px,
};

use crate::assets::icons;
use crate::db::roots::RootKind;
use crate::i18n::DirectionalStyle;
use crate::library::indexer::Wanted;
use crate::library::{self, Library, folders, root_text, sources};
use crate::settings;
use crate::theme::text_px;
use crate::theme::{self, Palette};
use crate::ui::button::Button;
use crate::ui::icon::icon;
use crate::ui::metrics;

/// The list's inner padding and the space between its rows.
const PADDING: f32 = 16.;
const GAP: f32 = 12.;
/// A folder's icon.
const FOLDER_ICON: f32 = 20.;

/// The Media Folders list.
pub(super) struct MediaFoldersPanel {
    /// Why the last folder picked wasn't added, until another is picked.
    refused: Option<SharedString>,
    /// Whether the library is used; off, the list is dimmed and does nothing.
    enabled: bool,
    _library: Subscription,
}

impl MediaFoldersPanel {
    /// The list, following the library's folder states.
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        let library = cx.observe_global::<Library>(|_, cx| cx.notify());
        Self { refused: None, enabled: library::is_enabled(cx), _library: library }
    }

    /// Dims the list while the library is off.
    pub(super) fn set_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.enabled = enabled;
        cx.notify();
    }

    fn set_refused(&mut self, refused: Option<String>, cx: &mut Context<Self>) {
        self.refused = refused.map(SharedString::from);
        cx.notify();
    }

    /// One folder's row.
    fn folder_row(&self, index: usize, folder: &Wanted, p: &Palette, cx: &Context<Self>) -> Div {
        let (glyph, section) = match folder.kind {
            RootKind::Music => (icons::MUSIC, tr!("Music")),
            RootKind::Video | RootKind::Mixed => (icons::VIDEOS, tr!("Videos")),
        };
        let status = tr!("{section} · {status}", section, status = self.status(&folder.path, cx));
        let text = div()
            .flex_1()
            .min_w_0()
            .flex_row_start()
            .flex_col()
            .child(
                div()
                    .text_color(p.text_strong)
                    .truncate()
                    .child(settings::display_path(&folder.path)),
            )
            .child(
                div()
                    .text_size(text_px(metrics::TEXT_SMALL))
                    .text_color(p.text_muted)
                    .child(status),
            );
        div()
            .flex_row_start()
            .items_center()
            .gap(px(GAP))
            .child(icon(glyph, FOLDER_ICON, p.icon))
            .child(text)
            .child(self.rescan_button(index, &folder.path, cx))
            .child(self.remove_button(index, &folder.path))
    }

    /// What the row says about the folder at `path`.
    fn status(&self, path: &Path, cx: &Context<Self>) -> SharedString {
        if !self.enabled {
            return tr!("Not indexed while the library is off").into();
        }
        match library::roots(cx).iter().find(|root| root.path == path) {
            Some(root) => root_text::status_text(root),
            None => tr!("Waiting to be indexed").into(),
        }
    }

    fn rescan_button(&self, index: usize, path: &Path, cx: &Context<Self>) -> Button {
        let scanning = library::roots(cx).iter().any(|r| r.path == path && r.indexing.is_some());
        let path = path.to_path_buf();
        Button::new(("rescan-folder", index), tr!("Rescan"))
            .icon(icons::REFRESH)
            .disabled(!self.enabled || scanning)
            .tooltip(tr!("Read every file in this folder again"), None::<SharedString>)
            .on_click(move |_: &ClickEvent, _, cx| library::resync(&path, cx))
    }

    fn remove_button(&self, index: usize, path: &Path) -> Button {
        let path = path.to_path_buf();
        Button::new(("remove-folder", index), tr!("Remove"))
            .icon(icons::REMOVE)
            .disabled(!self.enabled)
            .tooltip(tr!("Stop showing this folder in the library"), None::<SharedString>)
            .on_click(move |_: &ClickEvent, _, cx| remove(&path, cx))
    }

    /// The buttons that add a video or a music folder.
    fn add_buttons(&self, cx: &Context<Self>) -> Div {
        let this = cx.entity().downgrade();
        let shelves = [
            ("add-video-folder", tr!("Add Video Folder…"), RootKind::Video),
            ("add-music-folder", tr!("Add Music Folder…"), RootKind::Music),
        ];
        div().flex_row_start().gap(px(GAP)).children(shelves.map(|(id, label, kind)| {
            let this = this.clone();
            Button::new(id, label).icon(icons::ADD).disabled(!self.enabled).on_click(
                move |_: &ClickEvent, window: &mut Window, cx| {
                    this.update(cx, |panel, cx| panel.pick(kind, window, cx)).ok();
                },
            )
        }))
    }
}

impl Render for MediaFoldersPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let folders = sources::wanted(&settings::get(cx).local);
        let rows: Vec<Div> = folders
            .iter()
            .enumerate()
            .map(|(index, folder)| self.folder_row(index, folder, &p, cx))
            .collect();
        let empty = rows.is_empty().then(|| {
            div().text_color(p.text_muted).child(tr!("No media folders: the library is empty."))
        });
        let refused = self.refused.clone().map(|reason| {
            div().text_size(text_px(metrics::TEXT_SMALL)).text_color(p.danger).child(reason)
        });
        div()
            .w_full()
            .flex_row_start()
            .flex_col()
            .gap(px(GAP))
            .p(px(PADDING))
            .rounded(px(metrics::RADIUS))
            .bg(p.panel)
            .text_size(text_px(metrics::TEXT))
            .children(empty)
            .children(rows)
            .child(self.add_buttons(cx))
            .children(refused)
    }
}

/// Removes the media folder at `path` from the library.
fn remove(path: &Path, cx: &mut gpui::App) {
    let path: PathBuf = path.to_path_buf();
    settings::update(cx, |s| folders::remove(&mut s.local, &path));
}
