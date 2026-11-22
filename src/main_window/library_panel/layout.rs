//! How wide the library is and how its listing lays out at that width (S4.6.4, S4.10.1,
//! S4.10.2), decided without GPUI.
//!
//! The sidebar overlay is the rail and a 377 px panel, or the whole window when the window is
//! too narrow to leave the video room beside it. Full library mode is the labeled navigation and
//! the listing across the rest of the window. A list shows one item to a line in the overlay and
//! as many 400 px columns as fit in full library mode (`player_04`); a grid as many tiles as fit,
//! two in the overlay and 316 px wide in full library mode (`player_03`).

use super::row::ROW_HEIGHT;
use super::{INSET, PANEL_WIDTH, RAIL_WIDTH};
use crate::navigation::Presentation;
use crate::navigation::view::View;
use crate::ui::focus::Layout;
use crate::ui::list::grid_columns;

/// Below this window width the overlay takes the whole window (S4.10 edge case): beside the
/// sidebar the video would be narrower than the sidebar itself.
const NARROW_WINDOW: f32 = 2. * (RAIL_WIDTH + PANEL_WIDTH);
/// Full library mode's navigation: its margin from the window's edges, and its width
/// (`player_03`).
pub(super) const NAV_MARGIN: f32 = 8.;
pub(super) const NAV_WIDTH: f32 = 201.;
/// The narrowest a listing gets, however small the window.
const MIN_LIST_WIDTH: f32 = 120.;

/// The space inside a tile's frame around its contents (the selection's fill), the gap between
/// its picture and its name, and the name's line.
pub(super) const CELL_PAD: f32 = 4.;
pub(super) const NAME_GAP: f32 = 6.;
pub(super) const NAME_HEIGHT: f32 = 20.;

/// How a presentation spaces one view: the narrowest a column may get, the gap between
/// columns, and the gap between lines of tiles.
struct Spacing {
    min_cell: f32,
    gap: f32,
    line_gap: f32,
}

/// The overlay's grid: two tiles to its panel, 8 px apart.
const OVERLAY_GRID: Spacing = Spacing { min_cell: 164., gap: 8., line_gap: 8. };
/// Full library mode's grid: 316 px tiles 20 px apart in a 1920 px window.
const FULL_GRID: Spacing = Spacing { min_cell: 300., gap: 20., line_gap: 36. };
/// Full library mode's list: 400 px columns.
const FULL_LIST: Spacing = Spacing { min_cell: 380., gap: 20., line_gap: 0. };

/// The overlay's width in a window `window_width` wide: the rail and the panel, or the whole
/// window when it is narrow.
pub(super) fn overlay_width(window_width: f32) -> f32 {
    if window_width < NARROW_WINDOW { window_width } else { RAIL_WIDTH + PANEL_WIDTH }
}

/// The listing's width inside its side insets, in a window `window_width` wide.
pub(super) fn list_width(window_width: f32, presentation: Presentation) -> f32 {
    let panel = match presentation {
        Presentation::Overlay => overlay_width(window_width) - RAIL_WIDTH,
        Presentation::Full => window_width - nav_space(),
    };
    (panel - 2. * INSET).max(MIN_LIST_WIDTH)
}

/// The room full library mode's navigation takes from the window's left edge.
pub(super) fn nav_space() -> f32 {
    NAV_MARGIN + NAV_WIDTH + NAV_MARGIN
}

/// How the listing lays its items out: in lines of `columns` cells.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Shape {
    /// The items to a line.
    pub(super) columns: usize,
    /// Each item's width, and the gap between two of them.
    pub(super) cell_width: f32,
    pub(super) gap: f32,
    /// A line's height, with the gap under it.
    pub(super) line_height: f32,
    view: View,
}

impl Shape {
    /// The shape of `view` in a listing `width` wide, shown as `presentation`.
    pub(super) fn fit(width: f32, view: View, presentation: Presentation) -> Self {
        let spacing = match (view, presentation) {
            (View::List, Presentation::Overlay) => {
                let line_height = ROW_HEIGHT;
                return Self { columns: 1, cell_width: width, gap: 0., line_height, view };
            }
            (View::List, Presentation::Full) => FULL_LIST,
            (View::Grid, Presentation::Overlay) => OVERLAY_GRID,
            (View::Grid, Presentation::Full) => FULL_GRID,
        };
        let columns = grid_columns(width, spacing.min_cell, spacing.gap);
        let cell_width = (width - spacing.gap * (columns - 1) as f32) / columns as f32;
        let line_height = match view {
            View::List => ROW_HEIGHT,
            View::Grid => 2. * CELL_PAD + tile_height(cell_width) + NAME_GAP + NAME_HEIGHT,
        } + spacing.line_gap;
        Self { columns, cell_width, gap: spacing.gap, line_height, view }
    }

    /// One item to a line, as the sidebar's list.
    pub(super) fn column() -> Self {
        Self { columns: 1, cell_width: 0., gap: 0., line_height: ROW_HEIGHT, view: View::List }
    }

    /// Whether items show as tiles.
    pub(super) fn is_grid(&self) -> bool {
        self.view == View::Grid
    }

    /// Whether items sit side by side, so the keyboard moves through lines as well as items.
    pub(super) fn is_lines(&self) -> bool {
        self.columns > 1 || self.is_grid()
    }

    /// The line item `index` is on.
    pub(super) fn line_of(&self, index: usize) -> usize {
        index / self.columns
    }

    /// How many lines `len` items take.
    pub(super) fn lines(&self, len: usize) -> usize {
        len.div_ceil(self.columns)
    }

    /// A tile's picture: its width and its 16:9 height.
    pub(super) fn tile(&self) -> (f32, f32) {
        let width = self.cell_width - 2. * CELL_PAD;
        (width, width * 9. / 16.)
    }

    /// How the keyboard moves through the items when `shown` pixels of the listing show: a
    /// page key skips what fits on screen.
    pub(super) fn keys(&self, shown: f32) -> Layout {
        let page = ((shown / self.line_height) as usize).max(1);
        if self.is_lines() {
            Layout::grid(self.columns, page)
        } else {
            Layout { page, ..Layout::LIST }
        }
    }
}

/// A tile's picture height for a cell `cell_width` wide.
fn tile_height(cell_width: f32) -> f32 {
    (cell_width - 2. * CELL_PAD) * 9. / 16.
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn two_tiles_fill_the_overlay() {
        let width = list_width(1920., Presentation::Overlay);
        let shape = Shape::fit(width, View::Grid, Presentation::Overlay);
        assert_eq!(shape.columns, 2);
        assert!(near(2. * shape.cell_width + shape.gap, PANEL_WIDTH - 2. * INSET));
        let (tile_width, tile_height) = shape.tile();
        assert!(near(tile_width / tile_height, 16. / 9.));
    }

    #[test]
    fn full_library_mode_fits_the_designs_columns() {
        let width = list_width(1920., Presentation::Full);
        let grid = Shape::fit(width, View::Grid, Presentation::Full);
        assert_eq!(grid.columns, 5);
        assert!(near(grid.cell_width, 316.6));
        assert_eq!(Shape::fit(width, View::List, Presentation::Full).columns, 4);
    }

    #[test]
    fn a_narrow_window_gives_the_overlay_all_of_it() {
        assert!(near(overlay_width(1280.), RAIL_WIDTH + PANEL_WIDTH));
        assert!(near(overlay_width(600.), 600.));
        let width = list_width(600., Presentation::Overlay);
        assert!(near(width, 600. - RAIL_WIDTH - 2. * INSET));
    }

    #[test]
    fn a_tiny_window_still_has_one_column() {
        let width = list_width(100., Presentation::Full);
        assert!(near(width, MIN_LIST_WIDTH));
        assert_eq!(Shape::fit(width, View::Grid, Presentation::Full).columns, 1);
    }

    #[test]
    fn the_keyboard_moves_through_lines_where_items_sit_side_by_side() {
        let list = Shape::fit(337., View::List, Presentation::Overlay);
        assert!(!list.is_lines());
        assert_eq!(list.line_of(7), 7);
        let grid = Shape::fit(337., View::Grid, Presentation::Overlay);
        assert!(grid.is_lines());
        assert_eq!((grid.line_of(5), grid.lines(5)), (2, 3));
    }
}
