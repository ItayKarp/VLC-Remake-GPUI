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
    AnyElement, App, Context, EventEmitter, FocusHandle, Focusable, KeyDownEvent, ScrollStrategy,
    SharedString, UniformListScrollHandle, Window, accesskit, div, prelude::*, px, uniform_list,
};

use super::focus::{self, Axis, Layout};
use super::metrics;
use super::popover::BoundsTracker;
use crate::theme;

/// How the item at an index should look.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemState {
    pub selected: bool,
    /// The list has keyboard focus (so the selection shows a focus ring).
    pub focused: bool,
}

type RenderItem = Rc<dyn Fn(usize, ItemState, &mut Window, &mut App) -> AnyElement>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Columns {
    /// A list: one item per row.
    #[allow(dead_code)] // The library's list view (Stage 4).
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

pub fn row_count(len: usize, columns: usize) -> usize {
    len.div_ceil(columns.max(1))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Selected(usize),
    /// Enter or double-click.
    Activated(usize),
}

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
    pub fn new(
        label: impl Into<SharedString>,
        len: usize,
        columns: Columns,
        row_height: f32,
        render_item: impl Fn(usize, ItemState, &mut Window, &mut App) -> AnyElement + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        theme::observe(window, cx);
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

    #[allow(dead_code)] // Kit API for later stages.
    pub fn set_len(&mut self, len: usize, cx: &mut Context<Self>) {
        self.len = len;
        self.selected = self.selected.filter(|&s| s < len);
        cx.notify();
    }

    #[allow(dead_code)] // Kit API for later stages.
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

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
        let row_height = self.row_height;
        let len = self.len;
        let focused = self.focus_handle.is_focused(window);

        let list = uniform_list(
            "virtual-list",
            rows,
            cx.processor(move |this, range: Range<usize>, window, cx| {
                range
                    .map(|row| {
                        let items = row_items(row, columns, len).map(|index| {
                            let state =
                                ItemState { selected: this.selected == Some(index), focused };
                            let item = (this.render_item)(index, state, window, cx);
                            div()
                                .id(index)
                                .when_some(item_width, |d, w| d.w(px(w)).flex_none())
                                .when(item_width.is_none(), |d| d.flex_1().min_w_0())
                                .h_full()
                                .on_click(cx.listener(
                                    move |this, event: &gpui::ClickEvent, window, cx| {
                                        this.focus_handle.focus(window, cx);
                                        this.select(index, cx);
                                        if event.click_count() >= 2 {
                                            cx.emit(Event::Activated(index));
                                        }
                                    },
                                ))
                                .child(item)
                        });
                        div()
                            .id(("row", row))
                            .h(px(row_height))
                            .flex()
                            .flex_row()
                            .gap(px(gap))
                            .px(px(gap))
                            .children(items)
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .track_scroll(&self.scroll)
        .size_full();

        div()
            .id("virtual-list-frame")
            .relative()
            .size_full()
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
