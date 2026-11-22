//! The grid view (S4.6.4): tiles with the name below, as in full library mode (`player_03`),
//! two to a line in the sidebar overlay. A card's tile is its thumbnail with the list's watched
//! bar, hover play button and `•••`; a row's (a folder, a category's name, "+ Add") is its icon
//! on a plain tile, and it opens on one click as in the list.
//!
//! Each line of tiles is one item of the listing's `uniform_list`, so a grid of 10,000 cards
//! draws only the lines on screen.

use std::ops::Range;

use gpui::{AnyElement, ClickEvent, Context, Div, Stateful, div, prelude::*, px};

use super::card::{self, GROUP};
use super::drag;
use super::listing::{Body, Item, Items, card_key};
use super::row::{self, ROW_TEXT, Row};
use super::{INSET, MainWindow, PANEL_WIDTH};
use crate::db::media::MediaId;
use crate::db::thumbnails::SizeClass;
use crate::library::folder_view::Card;
use crate::theme::Palette;
use crate::ui::icon::icon;
use crate::ui::list::row_items;
use crate::ui::metrics;

/// Tiles to a line in the sidebar overlay.
pub(super) const COLUMNS: usize = 2;
/// The space between a line's tiles, and around each tile's contents (the selection's fill).
const GAP: f32 = 8.;
const CELL_PAD: f32 = 4.;
/// A tile's width with its padding, and its picture's: the panel's width shared out.
const CELL_WIDTH: f32 = (PANEL_WIDTH - 2. * INSET - GAP) / COLUMNS as f32;
const TILE_WIDTH: f32 = CELL_WIDTH - 2. * CELL_PAD;
/// Pictures are 16:9, as the grid's thumbnails are (`player_03`).
const TILE_HEIGHT: f32 = TILE_WIDTH * 9. / 16.;
/// The name's line under the picture, and the gap above it.
const NAME_GAP: f32 = 6.;
const NAME_HEIGHT: f32 = 20.;
/// The space between lines.
const LINE_GAP: f32 = 8.;
/// A line of tiles' height.
pub(super) const LINE_HEIGHT: f32 = 2. * CELL_PAD + TILE_HEIGHT + NAME_GAP + NAME_HEIGHT + LINE_GAP;
/// A row's icon on its tile.
const ROW_TILE_ICON: f32 = 32.;

impl MainWindow {
    /// The lines of tiles in `lines`, and the thumbnailer told what is on screen.
    pub(super) fn render_lines(
        &mut self,
        lines: Range<usize>,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let Body::Items(items) = self.listing(cx) else { return Vec::new() };
        let len = items.len();
        let on_screen: Vec<MediaId> = lines
            .clone()
            .flat_map(|line| row_items(line, COLUMNS, len))
            .filter_map(|index| match items.get(index)? {
                Item::Card(card) => card.media,
                Item::Row(_) => None,
            })
            .collect();
        let elements = lines.map(|line| self.render_line(line, &items, p, cx)).collect();
        self.show_thumbnails(on_screen, cx);
        elements
    }

    /// Line `line`: its items' tiles, side by side.
    fn render_line(
        &self,
        line: usize,
        items: &Items<'_>,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tiles = row_items(line, COLUMNS, items.len()).filter_map(|index| {
            Some(match items.get(index)? {
                Item::Row(row) => self.render_row_tile(index, row, p, cx),
                Item::Card(card) => self.render_card_tile(index, card, p, cx),
            })
        });
        let tiles: Vec<_> = tiles.collect();
        div().h(px(LINE_HEIGHT)).flex().gap(px(GAP)).children(tiles).into_any_element()
    }

    fn render_card_tile(
        &self,
        index: usize,
        card: &Card,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let selected = self.library_panel.selection.contains(&card_key(card));
        let picture =
            card::thumbnail_stack(card, SizeClass::Grid, (TILE_WIDTH, TILE_HEIGHT), p, cx);
        // `•••` sits on the picture's corner, over the hover's scrim, leaving the name the
        // tile's width.
        let more = div().absolute().top(px(CELL_PAD)).right_0().child(card::more_button(
            index,
            gpui::white(),
            cx,
        ));
        let picture = picture.child(more);
        let name = name_line().child(card::card_name(card, p));
        let cell = card::card_events(cell(("library-tile", index), selected, p), index, cx);
        drag::draggable(cell, index, cx).child(picture).child(name)
    }

    fn render_row_tile(
        &self,
        index: usize,
        row: &Row,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let selected = self.library_panel.selection.contains(&row.key());
        let (hover, strong) = (p.hover, p.text_strong);
        let target = row.target().clone();
        let picture = div()
            .w(px(TILE_WIDTH))
            .h(px(TILE_HEIGHT))
            .rounded(px(metrics::RADIUS))
            .bg(p.control)
            .flex()
            .items_center()
            .justify_center()
            .child(icon(row.icon(), ROW_TILE_ICON, p.icon));
        let picture = picture.relative().when(row.menu().is_some(), |picture| {
            let more = card::more_button(index, p.icon, cx);
            picture.child(div().absolute().top(px(CELL_PAD)).right_0().child(more))
        });
        let cell = drag::drop_target(cell(("library-row-tile", index), selected, p), row, p, cx);
        row::menu_events(cell, row, index, cx)
            .group(GROUP)
            .hover(move |style| style.bg(hover).text_color(strong))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.follow(target.clone(), window, cx);
            }))
            .child(picture)
            .child(name_line().child(div().min_w_0().truncate().child(row.label().clone())))
    }
}

/// A tile's frame: its width and padding, filled when `selected`.
fn cell(id: impl Into<gpui::ElementId>, selected: bool, p: &Palette) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .w(px(CELL_WIDTH))
        .p(px(CELL_PAD))
        .flex()
        .flex_col()
        .gap(px(NAME_GAP))
        .rounded(px(metrics::RADIUS))
        .text_size(px(ROW_TEXT))
        .text_color(p.text)
        .when(selected, |cell| cell.bg(p.selected))
}

/// The line under a tile's picture that holds its name.
fn name_line() -> Div {
    div().h(px(NAME_HEIGHT)).flex().items_center()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_tiles_fill_the_panel() {
        let used = COLUMNS as f32 * CELL_WIDTH + GAP;
        assert!((used - (PANEL_WIDTH - 2. * INSET)).abs() < f32::EPSILON);
        assert!((TILE_WIDTH / TILE_HEIGHT - 16. / 9.).abs() < 1e-4);
    }
}
