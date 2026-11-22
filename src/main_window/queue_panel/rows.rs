//! The Queue tab's rows (S3.2.3, S3.2.4): a virtualized list (10,000 entries draw only what's on
//! screen) of a drag handle, the name and the duration.
//!
//! The current entry's name is in the accent with an accent bar at its left edge (the design
//! has no highlight). Selected rows are filled. Rows drag to reorder (the selected ones together
//! when the dragged row is selected), and files dragged from the OS drop between rows; a line
//! shows the gap they'd land in. A drag dropped outside the rows is cancelled. Restored media that
//! wasn't there at launch is greyed (S3.4).

use std::collections::HashSet;
use std::ops::Range;

use gpui::{
    ClickEvent, Context, DragMoveEvent, ExternalPaths, Pixels, SharedString, Stateful, Window,
    accesskit, div, prelude::*, px, uniform_list,
};

use super::MainWindow;
use crate::app::media::Media;
use crate::assets::icons;
use crate::main_window::control_bar::clock::tabular_digits;
use crate::main_window::library_panel::DraggedMedia;
use crate::queue::{EntryId, Length, row_duration};
use crate::theme::{self, Palette};
use crate::ui::icon::icon;
use crate::ui::metrics;

/// A row's height, text size, drag handle size, and its gaps and padding.
const ROW_HEIGHT: f32 = 44.;
const TEXT: f32 = 14.;
const HANDLE: f32 = 24.;
const GAP: f32 = 12.;
const PADDING: f32 = 24.;
/// The current entry's bar, and the drop line's thickness.
const BAR_WIDTH: f32 = 3.;
const DROP_LINE: f32 = 2.;
/// The space around the empty queue's note.
const NOTE_PADDING: f32 = 24.;

/// Rows being dragged to a new place.
#[derive(Clone, Debug)]
pub(super) struct DraggedEntries {
    ids: HashSet<EntryId>,
    label: SharedString,
}

/// What follows the pointer while rows (or the library's media) are dragged: their name, or
/// "N items".
pub(in crate::main_window) struct DragPreview {
    label: SharedString,
}

impl DragPreview {
    /// A preview saying `label`.
    pub(in crate::main_window) fn new(label: SharedString) -> Self {
        Self { label }
    }
}

impl Render for DragPreview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        div()
            .px(px(GAP))
            .py(px(6.))
            .rounded(px(metrics::RADIUS))
            .bg(p.popover)
            .text_color(p.text)
            .text_size(px(TEXT))
            .shadow_md()
            .child(self.label.clone())
    }
}

/// The gap a drag at `y` (from the top of the rows' content, scrolled rows included) drops into:
/// the nearest boundary between rows, from 0 (above the first) to `len` (below the last).
pub(super) fn drop_slot(y: f32, len: usize) -> usize {
    if !y.is_finite() || y <= 0. {
        return 0;
    }
    ((y / ROW_HEIGHT).round() as usize).min(len)
}

impl MainWindow {
    /// The header and the rows.
    pub(super) fn render_queue_tab(&mut self, p: &Palette, cx: &mut Context<Self>) -> gpui::Div {
        if !cx.has_active_drag() {
            self.queue_panel.drop_slot = None;
        }
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(self.render_queue_header(p, cx))
            .child(self.render_rows(p, cx))
    }

    fn render_rows(&self, p: &Palette, cx: &mut Context<Self>) -> Stateful<gpui::Div> {
        let len = self.queue.len();
        let list = uniform_list(
            "queue-rows",
            len,
            cx.processor(|this, range: Range<usize>, window, cx| {
                let p = theme::palette(window, cx);
                range.map(|index| this.render_row(index, &p, cx)).collect::<Vec<_>>()
            }),
        )
        .track_scroll(&self.queue_panel.scroll)
        .size_full();
        let area = div()
            .id("queue-rows-area")
            .debug_selector(|| "queue-rows-area".into())
            .relative()
            .flex_1()
            .min_h_0()
            .role(accesskit::Role::List)
            .aria_label("Queue");
        Self::accept_drops(area, cx).child(list).children((len == 0).then(|| empty_note(p)))
    }

    /// Follows drags of rows, files and the library's media over `area` and takes their drops.
    fn accept_drops(area: Stateful<gpui::Div>, cx: &mut Context<Self>) -> Stateful<gpui::Div> {
        area.on_drag_move(cx.listener(|this, event: &DragMoveEvent<DraggedEntries>, _, cx| {
            this.track_drop_slot(event.event.position, event.bounds, cx);
        }))
        .on_drag_move(cx.listener(|this, event: &DragMoveEvent<ExternalPaths>, _, cx| {
            this.track_drop_slot(event.event.position, event.bounds, cx);
        }))
        .on_drop(cx.listener(|this, dragged: &DraggedEntries, window, cx| {
            this.drop_rows(&dragged.ids, window, cx);
        }))
        .on_drag_move(cx.listener(|this, event: &DragMoveEvent<DraggedMedia>, _, cx| {
            this.track_drop_slot(event.event.position, event.bounds, cx);
        }))
        .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
            this.drop_files(paths.paths(), window, cx);
        }))
        .on_drop(cx.listener(|this, dragged: &DraggedMedia, window, cx| {
            this.drop_files(dragged.paths(cx), window, cx);
        }))
    }

    /// Follows a drag over the rows: the gap under the pointer, or none outside them.
    fn track_drop_slot(
        &mut self,
        position: gpui::Point<Pixels>,
        bounds: gpui::Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let slot = bounds.contains(&position).then(|| {
            let scrolled = self.queue_panel.scroll.0.borrow().base_handle.offset().y;
            drop_slot(f32::from(position.y - bounds.top() - scrolled), self.queue.len())
        });
        if slot != self.queue_panel.drop_slot {
            self.queue_panel.drop_slot = slot;
            cx.notify();
        }
    }

    fn render_row(&self, index: usize, p: &Palette, cx: &mut Context<Self>) -> Stateful<gpui::Div> {
        let entry = &self.queue.entries()[index];
        let id = entry.id;
        let current = self.queue.current_index() == Some(index);
        let selected = self.queue_panel.selection.contains(id);
        let name = entry.media.display_name();
        let duration = match self.queue_panel.lengths.get(&entry.media) {
            Length::Known(duration) => row_duration(duration),
            _ => String::new(),
        };
        let row = div()
            .id(("queue-row", index))
            .w_full()
            .h(px(ROW_HEIGHT))
            .relative()
            .flex()
            .items_center()
            .gap(px(GAP))
            .px(px(PADDING))
            .text_size(px(TEXT))
            .role(accesskit::Role::ListItem)
            .aria_label(SharedString::from(name.clone()))
            .when(selected, |row| row.bg(p.selected))
            .when(!selected, |row| row.hover(|style| style.bg(p.hover)))
            .child(icon(icons::DRAG_HANDLE, HANDLE, p.icon))
            .child(row_name(name.clone(), self.row_look(&entry.media, current), p))
            .child(row_length(duration, p));
        self.mark_row(row, index, current, p)
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                this.click_row(id, event, window, cx);
            }))
            .on_drag(self.dragged(id, name), |dragged, _, _, cx| {
                let label = dragged.label.clone();
                cx.new(|_| DragPreview { label })
            })
    }

    /// How the name of a row playing `media` looks.
    fn row_look(&self, media: &Media, current: bool) -> Look {
        if current {
            Look::Current
        } else if self.queue_panel.missing.contains(media) {
            Look::Missing
        } else {
            Look::Normal
        }
    }

    /// Adds the current entry's bar and the drop line to the row at `index`.
    fn mark_row(
        &self,
        row: Stateful<gpui::Div>,
        index: usize,
        current: bool,
        p: &Palette,
    ) -> Stateful<gpui::Div> {
        let slot = self.queue_panel.drop_slot;
        let line_above = slot == Some(index);
        let line_below = slot == Some(index + 1) && index + 1 == self.queue.len();
        row.when(current, |row| row.child(edge(p).left_0().top_0().bottom_0().w(px(BAR_WIDTH))))
            .when(line_above, |row| row.child(edge(p).top_0().left_0().right_0().h(px(DROP_LINE))))
            .when(line_below, |row| {
                row.child(edge(p).bottom_0().left_0().right_0().h(px(DROP_LINE)))
            })
    }

    /// What dragging the row `id` (named `name`) carries: the selection when the row is in it,
    /// else just the row.
    fn dragged(&self, id: EntryId, name: String) -> DraggedEntries {
        let selection = &self.queue_panel.selection;
        if selection.contains(id) && selection.ids().len() > 1 {
            let count = selection.ids().len();
            DraggedEntries { ids: selection.ids().clone(), label: format!("{count} items").into() }
        } else {
            DraggedEntries { ids: HashSet::from([id]), label: name.into() }
        }
    }
}

/// How a row's name looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Look {
    Normal,
    /// The current entry: in the accent.
    Current,
    /// Restored media that wasn't there (S3.4): greyed.
    Missing,
}

/// A row's name, in the accent for the current entry and greyed for missing media.
fn row_name(name: String, look: Look, p: &Palette) -> gpui::Div {
    let color = match look {
        Look::Normal => p.text,
        Look::Current => p.accent_fg,
        Look::Missing => p.text_muted,
    };
    div().flex_1().min_w_0().truncate().text_color(color).child(name)
}

/// A row's duration, empty until it's known.
fn row_length(duration: String, p: &Palette) -> gpui::Div {
    div().flex_none().text_color(p.text_muted).font_features(tabular_digits()).child(duration)
}

/// What the rows say when the queue is empty.
fn empty_note(p: &Palette) -> gpui::Div {
    div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .p(px(NOTE_PADDING))
        .text_size(px(TEXT))
        .text_color(p.text_muted)
        .child("Nothing is queued. Drop files here or press + to add some.")
}

/// An accent strip placed absolutely on a row's edge.
fn edge(p: &Palette) -> gpui::Div {
    div().absolute().bg(p.accent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_land_in_the_nearest_gap() {
        assert_eq!(drop_slot(-5., 4), 0);
        assert_eq!(drop_slot(10., 4), 0);
        assert_eq!(drop_slot(30., 4), 1);
        assert_eq!(drop_slot(ROW_HEIGHT * 2.4, 4), 2);
        assert_eq!(drop_slot(ROW_HEIGHT * 50., 4), 4);
        assert_eq!(drop_slot(f32::NAN, 4), 0);
        assert_eq!(drop_slot(100., 0), 0);
    }
}
