//! Virtualized list and grid (S0.4.11) for libraries with 10k+ items.
//!
//! Built on `uniform_list`, which lays out only the visible rows. A grid is a
//! list of rows, each holding as many fixed-width items as fit the width
//! (measured on the previous frame). The collection is one tab stop: arrow keys,
//! Home/End and PageUp/PageDown move the selection (by row in a grid), Enter
//! activates it, and the selection scrolls into view.
//!
//! Items render through a callback, so the list holds no item data, just a count.

use std::ops::Range;
use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, EventEmitter, FocusHandle, Focusable, KeyDownEvent,
    ScrollStrategy, SharedString, Stateful, UniformListScrollHandle, Window, accesskit, div,
    prelude::*, px, uniform_list,
};

use super::focus::{self, Axis, Layout};
use super::metrics;
use super::popover::BoundsTracker;
use crate::theme;

/// How the item at an index should look.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemState {
    /// The item is the selected one.
    pub selected: bool,
    /// The list has keyboard focus (so the selection shows a focus ring).
    pub focused: bool,
}

/// Renders the item at an index in the given state.
type RenderItem = Rc<dyn Fn(usize, ItemState, &mut Window, &mut App) -> AnyElement>;

/// Whether the collection is a list or a grid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Columns {
    /// A list: one item per row.
    #[allow(dead_code, reason = "the library's list view (Stage 4)")]
    One,
    /// A grid of items `width` wide, `gap` apart.
    Fit { width: f32, gap: f32 },
}

/// How many `item_width` items fit in `width` with `gap` between them (at least 1).
pub fn grid_columns(width: f32, item_width: f32, gap: f32) -> usize {
    if !(width.is_finite() && item_width > 0.) {
        return 1;
    }
    (((width + gap) / (item_width + gap)).floor() as usize).max(1)
}

/// The item indices in `row`.
pub fn row_items(row: usize, columns: usize, len: usize) -> Range<usize> {
    let start = (row * columns).min(len);
    start..(start + columns).min(len)
}

/// How many rows `len` items take, `columns` to a row.
pub fn row_count(len: usize, columns: usize) -> usize {
    len.div_ceil(columns.max(1))
}

/// What a [`VirtualList`] tells its owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// The item at this index became the selection.
    Selected(usize),
    /// The item at this index was opened with Enter or a double-click.
    Activated(usize),
}

/// How many items a [`VirtualList`] holds and how it lays them out.
#[derive(Debug, Clone, Copy)]
pub struct Shape {
    /// The number of items.
    pub len: usize,
    /// How items are arranged across a row.
    pub columns: Columns,
    /// The height of one row, in pixels.
    pub row_height: f32,
}

/// A virtualized, keyboard-navigable list or grid of `len` items, rendered on demand.
pub struct VirtualList {
    len: usize,
    columns: Columns,
    row_height: f32,
    label: SharedString,
    selected: Option<usize>,
    focus_handle: FocusHandle,
    scroll: UniformListScrollHandle,
    viewport: BoundsTracker,
    render_item: RenderItem,
}

impl EventEmitter<Event> for VirtualList {}

impl Focusable for VirtualList {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl VirtualList {
    /// A collection laid out as `shape` describes, named `label` for screen readers.
    /// `render_item` draws an item when its row scrolls into view.
    pub fn new(
        label: impl Into<SharedString>,
        shape: Shape,
        render_item: impl Fn(usize, ItemState, &mut Window, &mut App) -> AnyElement + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        theme::observe(window, cx);
        let Shape { len, columns, row_height } = shape;
        Self {
            len,
            columns,
            row_height,
            label: label.into(),
            selected: None,
            focus_handle: cx.focus_handle().tab_stop(true),
            scroll: UniformListScrollHandle::new(),
            viewport: BoundsTracker::new(),
            render_item: Rc::new(render_item),
        }
    }

    /// Changes the item count, dropping a selection that no longer exists.
    #[allow(dead_code, reason = "kit API for later stages")]
    pub fn set_len(&mut self, len: usize, cx: &mut Context<Self>) {
        self.len = len;
        self.selected = self.selected.filter(|&s| s < len);
        cx.notify();
    }

    /// The selected item's index, if any.
    #[allow(dead_code, reason = "kit API for later stages")]
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// How many items fit in a row at the viewport's last measured width.
    fn column_count(&self) -> usize {
        match self.columns {
            Columns::One => 1,
            Columns::Fit { width, gap } => {
                let available =
                    self.viewport.bounds().map_or(width, |b| f32::from(b.size.width) - 2. * gap);
                grid_columns(available, width, gap)
            }
        }
    }

    fn visible_rows(&self) -> usize {
        self.viewport
            .bounds()
            .map_or(10, |b| (f32::from(b.size.height) / self.row_height) as usize)
            .max(1)
    }

    /// Selects the item at `index` and scrolls it into view; out-of-range indices are
    /// ignored.
    pub fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.len {
            return;
        }
        self.selected = Some(index);
        let row = index / self.column_count();
        self.scroll.scroll_to_item(row, ScrollStrategy::Nearest);
        cx.emit(Event::Selected(index));
        cx.notify();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "enter" && !event.keystroke.modifiers.modified() {
            if let Some(index) = self.selected {
                cx.emit(Event::Activated(index));
                cx.stop_propagation();
            }
            return;
        }
        let columns = self.column_count();
        let axis = if columns > 1 { Axis::Grid } else { Axis::Vertical };
        let Some(movement) = focus::group_key(event, axis) else {
            return;
        };
        let layout = Layout::grid(columns, self.visible_rows());
        if let Some(next) = focus::step(self.selected, self.len, movement, layout, |_| true) {
            cx.stop_propagation();
            if Some(next) != self.selected {
                self.select(next, cx);
            }
        }
    }

    /// One row: as many items as fit, side by side.
    fn render_row(
        &self,
        row: usize,
        layout: RowLayout,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let items = row_items(row, layout.columns, layout.len)
            .map(|index| self.render_cell(index, layout, window, cx));
        div()
            .id(("row", row))
            .h(px(layout.row_height))
            .flex()
            .flex_row()
            .gap(px(layout.gap))
            .px(px(layout.gap))
            .children(items)
    }

    /// The clickable cell holding the item at `index`.
    fn render_cell(
        &self,
        index: usize,
        layout: RowLayout,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let state = ItemState { selected: self.selected == Some(index), focused: layout.focused };
        let item = (self.render_item)(index, state, window, cx);
        let item_width = layout.item_width;
        div()
            .id(index)
            .when_some(item_width, |d, w| d.w(px(w)).flex_none())
            .when(item_width.is_none(), |d| d.flex_1().min_w_0())
            .h_full()
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                this.focus_handle.focus(window, cx);
                this.select(index, cx);
                if event.click_count() >= 2 {
                    cx.emit(Event::Activated(index));
                }
            }))
            .child(item)
    }
}

/// What every row needs to know about this frame's layout.
#[derive(Clone, Copy)]
struct RowLayout {
    columns: usize,
    len: usize,
    /// `None` for a list, whose items fill the row.
    item_width: Option<f32>,
    gap: f32,
    row_height: f32,
    /// Whether the list has keyboard focus.
    focused: bool,
}

impl Render for VirtualList {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let columns = self.column_count();
        let rows = row_count(self.len, columns);
        let (item_width, gap) = match self.columns {
            Columns::One => (None, 0.),
            Columns::Fit { width, gap } => (Some(width), gap),
        };
        let layout = RowLayout {
            columns,
            len: self.len,
            item_width,
            gap,
            row_height: self.row_height,
            focused: self.focus_handle.is_focused(window),
        };

        let list = uniform_list(
            "virtual-list",
            rows,
            cx.processor(move |this, range: Range<usize>, window, cx| {
                range.map(|row| this.render_row(row, layout, window, cx)).collect::<Vec<_>>()
            }),
        )
        .track_scroll(&self.scroll)
        .size_full();

        div()
            .id("virtual-list-frame")
            .relative()
            .size_full()
            .key_context(focus::CONTROL)
            .track_focus(&self.focus_handle)
            .role(if item_width.is_some() { accesskit::Role::Grid } else { accesskit::Role::List })
            .aria_label(self.label.clone())
            .rounded(px(metrics::RADIUS))
            .border_2()
            .border_color(gpui::transparent_black())
            .on_key_down(cx.listener(Self::on_key_down))
            .child(self.viewport.element())
            .child(list)
            .map(|frame| focus::ring(frame, &p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_fit_the_width() {
        // 3 items of 100 with 10 gaps need 320.
        assert_eq!(grid_columns(320., 100., 10.), 3);
        assert_eq!(grid_columns(319., 100., 10.), 2);
        assert_eq!(grid_columns(50., 100., 10.), 1);
        assert_eq!(grid_columns(f32::NAN, 100., 10.), 1);
        assert_eq!(grid_columns(500., 0., 10.), 1);
    }

    #[test]
    fn rows_cover_every_item() {
        assert_eq!(row_count(10_000, 7), 1429);
        assert_eq!(row_items(1428, 7, 10_000), 9996..10_000);
        assert_eq!(row_items(0, 7, 3), 0..3);
        assert_eq!(row_count(0, 7), 0);
        assert_eq!(row_items(5, 7, 3), 3..3);
    }
}
