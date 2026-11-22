//! Dragging media out of the library (S4.6.7): a card drags its file, or every selected card's
//! when it is selected. Dropped on the queue, the files go where they land; on a playlist's row,
//! into that playlist. Outside the window the drag becomes the OS's, offering the files to
//! other apps (a file manager copies them, a mail app attaches them).
//!
//! Holding a drag over a rail icon shows that section, so a playlist can be reached mid-drag:
//! over Playlists, then onto the playlist.

use std::cell::OnceCell;
use std::path::PathBuf;

use gpui::{
    App, Context, DragMoveEvent, ExternalDragPayload, FileDragPaths, SharedString, Stateful,
    WeakEntity, Window, prelude::*,
};

use super::MainWindow;
use super::listing::{Body, Item, card_key};
use super::row::{Row, Target};
use crate::app::media::Media;
use crate::main_window::queue_panel::DragPreview;
use crate::navigation::route::{Route, Section};
use crate::playlists;
use crate::theme::Palette;

/// Media dragged from the library: the cards dragged, resolved once when the drag starts, so
/// the library may show another place meanwhile.
pub(in crate::main_window) struct DraggedMedia {
    /// The listing item the drag started on.
    index: usize,
    main: WeakEntity<MainWindow>,
    paths: OnceCell<Vec<PathBuf>>,
}

impl DraggedMedia {
    /// The dragged files, in list order.
    pub(in crate::main_window) fn paths(&self, cx: &App) -> &[PathBuf] {
        self.paths.get_or_init(|| {
            let main = self.main.upgrade();
            main.map_or_default(|main| main.read(cx).dragged_paths(self.index, cx))
        })
    }

    /// What the preview says: the file's name, or "N items".
    fn label(&self, cx: &App) -> SharedString {
        match self.paths(cx) {
            [path] => path.file_name().unwrap_or_default().to_string_lossy().into_owned().into(),
            paths => format!("{} items", paths.len()).into(),
        }
    }
}

impl MainWindow {
    /// What dragging item `index` carries: the selected cards when it is one of them, else it.
    fn dragged_paths(&self, index: usize, cx: &App) -> Vec<PathBuf> {
        let Body::Items(items) = self.listing(cx) else { return Vec::new() };
        let Some(Item::Card(card)) = items.get(index) else { return Vec::new() };
        let selection = &self.library_panel.selection;
        if !selection.contains(&card_key(card)) {
            return vec![card.path.clone()];
        }
        let cards = items.cards.iter().filter(|card| selection.contains(&card_key(card)));
        cards.map(|card| card.path.clone()).collect()
    }
}

/// Adds the dragged media to the playlist `route` shows, if it shows one.
fn drop_on_playlist(
    route: &Route,
    dragged: &DraggedMedia,
    window: &mut Window,
    cx: &mut Context<MainWindow>,
) {
    let Route::Playlists(Some(id)) = route else { return };
    let summaries = playlists::summaries(cx);
    let Some(summary) = summaries.iter().find(|p| p.id == *id).cloned() else { return };
    let media: Vec<Media> = dragged.paths(cx).iter().cloned().map(Media::File).collect();
    MainWindow::add_to_playlist(&summary, &media, window, cx);
}

/// `frame` as the card at item `index`, which can be dragged.
pub(super) fn draggable(
    frame: Stateful<gpui::Div>,
    index: usize,
    cx: &Context<MainWindow>,
) -> Stateful<gpui::Div> {
    let dragged = DraggedMedia { index, main: cx.entity().downgrade(), paths: OnceCell::new() };
    frame
        .debug_selector(|| format!("library-card-{index}"))
        .on_drag(dragged, |dragged, _, _, cx| {
            let label = dragged.label(cx);
            cx.new(|_| DragPreview::new(label))
        })
        .external_drag_payload(|dragged: &DraggedMedia, _, cx| {
            let files = dragged.paths(cx).iter().map(|path| (path.clone(), false));
            Some(ExternalDragPayload::Files(FileDragPaths::new(files)))
        })
}

/// `frame` as `row`: a playlist's row takes dragged media, and is tinted while they are over
/// it. Other rows are left as they are.
pub(super) fn drop_target(
    frame: Stateful<gpui::Div>,
    row: &Row,
    p: &Palette,
    cx: &mut Context<MainWindow>,
) -> Stateful<gpui::Div> {
    let Target::Go(route @ Route::Playlists(Some(_))) = row.target() else { return frame };
    let (route, tint) = (route.clone(), p.accent_tint);
    frame.drag_over::<DraggedMedia>(move |style, _, _, _| style.bg(tint)).on_drop(cx.listener(
        move |_, dragged: &DraggedMedia, window, cx| {
            drop_on_playlist(&route, dragged, window, cx);
        },
    ))
}

/// `button` as `section`'s rail button: holding a drag over it shows that section.
pub(super) fn spring_loaded(
    button: gpui::Div,
    section: Section,
    cx: &mut Context<MainWindow>,
) -> gpui::Div {
    button.debug_selector(move || format!("rail-{}", section.label())).on_drag_move(cx.listener(
        move |this, event: &DragMoveEvent<DraggedMedia>, _, cx| {
            let over = event.bounds.contains(&event.event.position);
            if over && this.library_panel.history.current().section() != section {
                this.navigate(section.root(), cx);
            }
        },
    ))
}
