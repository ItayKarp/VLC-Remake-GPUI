//! What the sidebar lists for its route, until the browsing views replace it (S4.6): the places
//! a route leads to (media folders, subfolders, Music's categories, playlists) and the recently
//! played media (History). Subfolders are read in the background; a folder that is gone says
//! "This folder is no longer available" with Back (S4.5 edge case).

use std::io;
use std::path::{Path, PathBuf};

use gpui::{
    App, ClickEvent, Context, Div, SharedString, Stateful, Task, Window, div, prelude::*, px,
};

use super::MainWindow;
use crate::app::media::Media;
use crate::app::request::OpenRequest;
use crate::assets::icons;
use crate::navigation::history::Step;
use crate::navigation::route::{MusicCategory, MusicRoute, Route};
use crate::opening::recent;
use crate::playlists;
use crate::settings;
use crate::theme::Palette;
use crate::ui::button::Button;
use crate::ui::{focus, icon, metrics};

/// A row's height, its icon box and the gap before its text (`player_02`).
const ROW_HEIGHT: f32 = 42.;
const ROW_ICON_BOX: f32 = 32.;
const ROW_GAP: f32 = 10.;
const ROW_TEXT: f32 = 14.;
/// Air between the header's divider and the first row.
const LIST_TOP: f32 = 15.;
/// The panel's side padding.
pub(super) const INSET: f32 = 20.;

/// A folder's subfolders, once read.
pub(in crate::main_window) struct FolderListing {
    folder: PathBuf,
    state: FolderState,
    _read: Task<()>,
}

enum FolderState {
    Reading,
    Read(Vec<PathBuf>),
    Gone,
}

/// One row: what it shows and what clicking it does.
struct Row {
    label: SharedString,
    icon: &'static str,
    target: Target,
}

#[derive(Clone)]
enum Target {
    Go(Route),
    Play(Media),
}

/// What the body shows.
enum Body {
    Rows(Vec<Row>),
    /// The folder is still being read.
    Reading,
    /// The route's folder or playlist is gone.
    Gone(&'static str),
}

impl MainWindow {
    /// Reads the current route's folder again, if it shows one and the sidebar is open.
    pub(super) fn reload_folder(&mut self, cx: &mut Context<Self>) {
        let panel = &mut self.library_panel;
        let Some(folder) = panel.history.current().folder().filter(|_| panel.open).cloned() else {
            panel.folder = None;
            return;
        };
        // The same folder read again keeps its rows meanwhile, so they don't flicker.
        let state = match panel.folder.take() {
            Some(old) if old.folder == folder => old.state,
            _ => FolderState::Reading,
        };
        let read = read_folder(folder.clone(), cx);
        self.library_panel.folder = Some(FolderListing { folder, state, _read: read });
    }

    /// The rows under the header, or why there are none.
    pub(super) fn render_listing(&self, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let body = div().id("library-listing").flex_1().min_h_0().overflow_y_scroll();
        match self.listing(cx) {
            Body::Rows(rows) if rows.is_empty() => body.child(note("Nothing here yet.", p)),
            Body::Rows(rows) => body.pt(px(LIST_TOP)).children(
                rows.into_iter().enumerate().map(|(index, row)| render_row(index, row, p, cx)),
            ),
            Body::Reading => body,
            Body::Gone(message) => body.child(self.render_gone(message, p, cx)),
        }
    }

    /// How many items the route lists, for the header's count.
    pub(super) fn listed_count(&self, cx: &App) -> usize {
        match self.listing(cx) {
            Body::Rows(rows) => rows.len(),
            Body::Reading | Body::Gone(_) => 0,
        }
    }

    fn listing(&self, cx: &App) -> Body {
        let route = self.library_panel.history.current();
        if let Some(listing) = &self.library_panel.folder {
            return match &listing.state {
                FolderState::Reading => Body::Reading,
                FolderState::Read(folders) => Body::Rows(folder_rows(route, folders)),
                FolderState::Gone => Body::Gone("This folder is no longer available."),
            };
        }
        if !super::exists(route, cx) {
            return Body::Gone("This playlist no longer exists.");
        }
        Body::Rows(rows(route, cx))
    }

    fn render_gone(&self, message: &'static str, p: &Palette, cx: &mut Context<Self>) -> Div {
        let back = Button::new("library-gone-back", "Back")
            .disabled(!self.can_step_library(Step::Back, cx))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.step_library(Step::Back, cx);
            }));
        note(message, p).flex().flex_col().items_start().gap(px(12.)).child(back)
    }
}

/// Reads `folder`'s subfolders in the background into the listing, if it still shows `folder`.
fn read_folder(folder: PathBuf, cx: &mut Context<MainWindow>) -> Task<()> {
    let show_hidden = settings::get(cx).prefs.library.show_hidden_files;
    let path = folder.clone();
    let read = cx.background_spawn(async move { subfolders(&path, show_hidden) });
    cx.spawn(async move |this, cx| {
        let state = match read.await {
            Ok(folders) => FolderState::Read(folders),
            Err(_) => FolderState::Gone,
        };
        this.update(cx, |this, cx| {
            let listing = this.library_panel.folder.as_mut();
            if let Some(listing) = listing.filter(|listing| listing.folder == folder) {
                listing.state = state;
                cx.notify();
            }
        })
        .ok();
    })
}

/// The rows a route that shows no folder lists.
fn rows(route: &Route, cx: &App) -> Vec<Row> {
    let local = &settings::get(cx).local;
    match route {
        Route::Videos(None) => folder_rows(route, &local.media_folders),
        Route::Music(MusicRoute::Songs(None)) => folder_rows(route, &local.music_folders),
        Route::Music(MusicRoute::Categories) => MusicCategory::ALL
            .into_iter()
            .map(|category| Row {
                label: category.label().into(),
                icon: category_icon(category),
                target: Target::Go(category.route()),
            })
            .collect(),
        Route::Playlists(None) => playlists::summaries(cx)
            .iter()
            .map(|summary| Row {
                label: summary.title.clone().into(),
                icon: icons::PLAYLIST,
                target: Target::Go(Route::Playlists(Some(summary.id))),
            })
            .collect(),
        Route::History => recent::items(cx).into_iter().map(recent_row).collect(),
        _ => Vec::new(),
    }
}

fn folder_rows(route: &Route, folders: &[PathBuf]) -> Vec<Row> {
    let row = |folder: &PathBuf| {
        let target = route.with_folder(folder.clone())?;
        let label = Media::File(folder.clone()).display_name().into();
        Some(Row { label, icon: icons::FOLDER, target: Target::Go(target) })
    };
    folders.iter().filter_map(row).collect()
}

fn recent_row(media: Media) -> Row {
    let icon = if media.as_path().is_some() { icons::FILE } else { icons::STREAMING };
    Row { label: media.display_name().into(), icon, target: Target::Play(media) }
}

fn category_icon(category: MusicCategory) -> &'static str {
    match category {
        MusicCategory::Songs => icons::MUSIC,
        MusicCategory::Albums => icons::ALBUM,
        MusicCategory::Artists => icons::ARTIST,
        MusicCategory::Genres => icons::GENRE,
    }
}

fn render_row(index: usize, row: Row, p: &Palette, cx: &mut Context<MainWindow>) -> Stateful<Div> {
    let (hover, strong) = (p.hover, p.text_strong);
    let target = row.target;
    let element = div()
        .id(("library-row", index))
        .h(px(ROW_HEIGHT))
        .mx(px(INSET))
        .flex()
        .items_center()
        .gap(px(ROW_GAP))
        .rounded(px(metrics::RADIUS))
        .border_2()
        .border_color(gpui::transparent_black())
        .tab_index(0)
        .cursor_pointer()
        .text_size(px(ROW_TEXT))
        .hover(move |style| style.bg(hover).text_color(strong))
        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
            this.follow(target.clone(), window, cx);
        }))
        .child(
            div()
                .size(px(ROW_ICON_BOX))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .child(icon::icon(row.icon, metrics::ICON, p.icon)),
        )
        .child(div().min_w_0().truncate().child(row.label));
    focus::ring(element, p)
}

impl MainWindow {
    /// Does what a row says: goes to its route or plays its media.
    fn follow(&mut self, target: Target, window: &mut Window, cx: &mut Context<Self>) {
        match target {
            Target::Go(route) => self.navigate(route, cx),
            Target::Play(media) => self.open(OpenRequest::replace(vec![media]), window, cx),
        }
    }
}

/// A muted line of text in the panel.
fn note(text: &'static str, p: &Palette) -> Div {
    div().p(px(INSET)).text_size(px(ROW_TEXT)).text_color(p.text_muted).child(text)
}

/// `folder`'s subfolders, by name ignoring case; hidden ones only when the library shows them.
fn subfolders(folder: &Path, show_hidden: bool) -> io::Result<Vec<PathBuf>> {
    let mut folders: Vec<PathBuf> = std::fs::read_dir(folder)?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter(|entry| show_hidden || !entry.file_name().to_string_lossy().starts_with('.'))
        .map(|entry| entry.path())
        .collect();
    folders.sort_by_cached_key(|path| path.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    Ok(folders)
}
