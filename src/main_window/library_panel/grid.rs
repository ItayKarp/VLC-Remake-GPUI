//! Lines of items side by side (S4.6.4, S4.10.2): the grid view's tiles with the name below, two
//! to a line in the sidebar overlay and as many as fit in full library mode (`player_03`), and
//! full library mode's list in columns (`player_04`). A card's tile is its thumbnail with the
//! list's watched bar, hover play button and `•••`; a row's (a folder, a category's name, "+
//! Add") is its icon on a plain tile, and it opens on one click as in the list.
//!
//! Each line is one item of the listing's `uniform_list`, so a grid of 10,000 cards draws only
//! the lines on screen. [`layout`](super::layout) decides how many items a line holds.

use std::ops::Range;

use gpui::{AnyElement, ClickEvent, Context, Div, Stateful, div, prelude::*, px};

use super::MainWindow;
use super::card::{self, GROUP};
use super::drag;
use super::layout::{CELL_PAD, NAME_GAP, NAME_HEIGHT};
use super::listing::{Body, Item, Items, card_key};
use super::row::{self, ROW_TEXT, Row};
use crate::db::media::MediaId;
use crate::db::thumbnails::SizeClass;
use crate::i18n::DirectionalStyle;
use crate::library::folder_view::Card;
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::icon::icon;
use crate::ui::list::row_items;
use crate::ui::metrics;

/// A row's icon on its tile.
const ROW_TILE_ICON: f32 = 32.;

impl MainWindow {
    /// The lines in `lines`, and the thumbnailer told what is on screen.
    pub(super) fn render_lines(
        &mut self,
        lines: Range<usize>,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let Body::Items(items) = self.listing(cx) else { return Vec::new() };
        let shape = self.library_shape(cx);
        let len = items.len();
        let on_screen: Vec<MediaId> = lines
            .clone()
            .flat_map(|line| row_items(line, shape.columns, len))
            .filter_map(|index| match items.get(index)? {
                Item::Card(card) => card.media,
                Item::Row(_) => None,
            })
            .collect();
        let elements = lines.map(|line| self.render_line(line, &items, p, cx)).collect();
        self.show_thumbnails(on_screen, cx);
        elements
    }

    /// Line `line`: its items side by side, as tiles or as list rows.
    fn render_line(
        &self,
        line: usize,
        items: &Items<'_>,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let shape = self.library_shape(cx);
        let cells = row_items(line, shape.columns, items.len()).filter_map(|index| {
            let item = items.get(index)?;
            Some(match item {
                Item::Row(row) if shape.is_grid() => {
                    self.render_row_tile(Tile::new(index, shape.tile()), row, p, cx)
                }
                Item::Card(card) if shape.is_grid() => {
                    self.render_card_tile(Tile::new(index, shape.tile()), card, p, cx)
                }
                item => self.render_list_cell(index, &item, p, cx),
            })
        });
        let cells: Vec<_> = cells.collect();
        div()
            .h(px(shape.line_height))
            .flex_row_start()
            .gap(px(shape.gap))
            .children(cells)
            .into_any_element()
    }

    /// Item `index` as a list row in its column (full library mode's list, `player_04`).
    fn render_list_cell(
        &self,
        index: usize,
        item: &Item<'_>,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let element = match item {
            Item::Row(row) => {
                let selected = self.library_panel.selection.contains(&row.key());
                row.render(index, selected, p, cx).into_any_element()
            }
            Item::Card(card) => self.render_card(index, card, p, cx).into_any_element(),
        };
        let width = self.library_shape(cx).cell_width;
        div().flex_none().w(px(width)).child(element).into_any_element()
    }

    /// A card's tile: its picture with `•••` on its corner, and its name; on Home its artist
    /// under that.
    pub(super) fn render_card_tile(
        &self,
        tile: Tile,
        card: &Card,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let index = tile.index;
        let selected = self.library_panel.selection.contains(&card_key(card));
        let picture = card::thumbnail_stack(card, SizeClass::Grid, tile.picture, p, cx);
        // `•••` sits on the picture's corner, over the hover's scrim, leaving the name the
        // tile's width.
        let more = div().absolute().top(px(CELL_PAD)).inset_end(px(0.)).child(card::more_button(
            index,
            gpui::white(),
            cx,
        ));
        let picture = picture.child(more);
        let name = name_line().child(card::card_name(card, p));
        let artist = card.tags.artist.clone().filter(|_| tile.artist);
        let artist = artist.map(|artist| {
            name_line().text_color(p.text_muted).child(div().min_w_0().truncate().child(artist))
        });
        let frame = cell(("library-tile", index), tile.width(), selected, p);
        let cell = card::card_events(frame, index, cx);
        let cell = drag::draggable(cell, index, cx).child(picture).child(name);
        cell.children(artist).into_any_element()
    }

    /// A row's tile: its icon on a plain picture, and its name; one click follows it.
    pub(super) fn render_row_tile(
        &self,
        tile: Tile,
        row: &Row,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let index = tile.index;
        let selected = self.library_panel.selection.contains(&row.key());
        let (tile_width, tile_height) = tile.picture;
        let (hover, strong) = (p.hover, p.text_strong);
        let target = row.target().clone();
        let picture = div()
            .w(px(tile_width))
            .h(px(tile_height))
            .rounded(px(metrics::RADIUS))
            .bg(p.control)
            .flex_row_start()
            .items_center()
            .justify_center()
            .child(icon(row.icon(), ROW_TILE_ICON, p.icon));
        let picture = picture.relative().when(row.menu().is_some(), |picture| {
            let more = card::more_button(index, p.icon, cx);
            picture.child(div().absolute().top(px(CELL_PAD)).inset_end(px(0.)).child(more))
        });
        let frame = cell(("library-row-tile", index), tile.width(), selected, p);
        let cell = drag::drop_target(frame, row, p, cx);
        row::menu_events(cell, row, index, cx)
            .group(GROUP)
            .hover(move |style| style.bg(hover).text_color(strong))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.follow(target.clone(), window, cx);
            }))
            .child(picture)
            .child(name_line().child(div().min_w_0().truncate().child(row.label().clone())))
            .into_any_element()
    }
}

/// Which item a tile draws and how big its picture is.
#[derive(Clone, Copy, Debug)]
pub(super) struct Tile {
    /// The item's index in the listing.
    pub(super) index: usize,
    /// The picture's width and height.
    pub(super) picture: (f32, f32),
    /// A song's artist shows under its name (Home's Listen Again, `player_07`).
    pub(super) artist: bool,
}

impl Tile {
    /// Item `index`'s tile with a `picture` sized picture.
    pub(super) fn new(index: usize, picture: (f32, f32)) -> Self {
        Self { index, picture, artist: false }
    }

    /// The tile's width with its padding.
    fn width(self) -> f32 {
        self.picture.0 + 2. * CELL_PAD
    }
}

/// A tile's frame: `width` wide with its padding, filled when `selected`.
fn cell(id: impl Into<gpui::ElementId>, width: f32, selected: bool, p: &Palette) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .w(px(width))
        .p(px(CELL_PAD))
        .flex_row_start()
        .flex_col()
        .gap(px(NAME_GAP))
        .rounded(px(metrics::RADIUS))
        .text_size(text_px(ROW_TEXT))
        .text_color(p.text)
        .when(selected, |cell| cell.bg(p.selected))
}

/// The line under a tile's picture that holds its name.
fn name_line() -> Div {
    div().h(px(NAME_HEIGHT)).flex_row_start().items_center()
}
