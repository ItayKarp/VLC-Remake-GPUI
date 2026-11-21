//! Keyboard focus model (S0.4.14).
//!
//! - **Tab / Shift+Tab** move through tab stops in `tab_index` order. Every window
//!   root calls [`tab_navigation`]; a modal traps it ([`super::modal`]).
//! - **Arrow keys** move inside a group (theme tiles, swatches, segmented values,
//!   tabs, menus, grids). A group is a single tab stop, so Tab leaves it instead of
//!   walking every item; inside it the arrows move the selection (radio groups,
//!   tabs) or the highlight (menus, lists, grids). [`step`] computes where a move
//!   lands.
//! - The **focus ring** shows only for keyboard focus (`focus_visible`), never after
//!   a mouse click. Components draw it with [`ring`].

use gpui::{InteractiveElement, KeyDownEvent, Styled, actions};

use crate::theme::Palette;

actions!(
    ui,
    [
        /// Move focus to the next tab stop (Tab).
        FocusNext,
        /// Move focus to the previous tab stop (Shift+Tab).
        FocusPrev,
    ]
);

/// Handles Tab / Shift+Tab on a window's root element.
pub fn tab_navigation<E: InteractiveElement>(element: E) -> E {
    element
        .on_action(|_: &FocusNext, window, cx| window.focus_next(cx))
        .on_action(|_: &FocusPrev, window, cx| window.focus_prev(cx))
}

/// Draws the keyboard focus ring as a 2px border in the focus color. Give the
/// element a transparent border of the same width so focusing doesn't shift it,
/// or let it already have a border (wells): the ring recolors it.
pub fn ring<E: InteractiveElement + Styled>(element: E, p: &Palette) -> E {
    let color = p.focus_ring;
    element.focus_visible(move |style| style.border_color(color))
}

/// A movement inside a group, from an arrow key or Home/End.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Prev,
    Next,
    Up,
    Down,
    First,
    Last,
    PageUp,
    PageDown,
}

impl Move {
    /// The movement for a key in a group laid out along `axis`. Groups on one axis
    /// accept both arrow pairs (Left/Up go back), like native radio groups.
    pub fn from_key(key: &str, axis: Axis) -> Option<Move> {
        Some(match (key, axis) {
            ("left", Axis::Grid) => Move::Prev,
            ("right", Axis::Grid) => Move::Next,
            ("up", Axis::Grid) => Move::Up,
            ("down", Axis::Grid) => Move::Down,
            ("left" | "up", _) => Move::Prev,
            ("right" | "down", _) => Move::Next,
            ("home", _) => Move::First,
            ("end", _) => Move::Last,
            ("pageup", _) => Move::PageUp,
            ("pagedown", _) => Move::PageDown,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
    /// Rows of `columns` items: Left/Right step, Up/Down jump a row.
    Grid,
}

/// Where a movement lands in a group of `len` items.
///
/// - `columns` is the grid width (1 for lists).
/// - `page` is how many items PageUp/PageDown skip.
/// - `wrap` cycles past the ends (menus, swatches); lists and grids stop.
/// - `enabled` skips disabled items; if nothing is enabled the result is `None`.
pub fn step(
    current: Option<usize>,
    len: usize,
    movement: Move,
    layout: Layout,
    enabled: impl Fn(usize) -> bool,
) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let Layout { columns, page, wrap } = layout;
    let columns = columns.max(1);
    let last = len - 1;
    let first_enabled = (0..len).find(|&i| enabled(i))?;
    let last_enabled = (0..len).rev().find(|&i| enabled(i))?;

    let Some(current) = current.filter(|&c| c < len) else {
        // Nothing focused yet: forward moves start at the top, backward ones at
        // the bottom.
        return Some(match movement {
            Move::Prev | Move::Up | Move::Last | Move::PageUp => last_enabled,
            _ => first_enabled,
        });
    };

    // A single jump, then walk on in the same direction past disabled items.
    let (target, direction): (isize, isize) = match movement {
        Move::First => return Some(first_enabled),
        Move::Last => return Some(last_enabled),
        Move::Prev => (current as isize - 1, -1),
        Move::Next => (current as isize + 1, 1),
        Move::Up => (current as isize - columns as isize, -(columns as isize)),
        Move::Down => (current as isize + columns as isize, columns as isize),
        Move::PageUp => (current as isize - page.max(1) as isize, -1),
        Move::PageDown => (current as isize + page.max(1) as isize, 1),
    };
    let paging = matches!(movement, Move::PageUp | Move::PageDown);

    let mut index = target;
    for _ in 0..=len {
        if index < 0 || index > last as isize {
            if paging {
                // Paging past an end lands on it.
                return Some(if index < 0 { first_enabled } else { last_enabled });
            }
            if !wrap {
                return Some(current);
            }
            index = index.rem_euclid(len as isize);
        }
        if enabled(index as usize) {
            return Some(index as usize);
        }
        index += direction;
    }
    Some(current)
}

/// How a group is laid out, for [`step`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub columns: usize,
    pub page: usize,
    pub wrap: bool,
}

impl Layout {
    /// A one-dimensional group that wraps around (menus, swatches, tabs).
    pub const WRAPPING: Layout = Layout { columns: 1, page: 10, wrap: true };
    /// A one-dimensional group that stops at the ends (lists, dropdowns).
    pub const LIST: Layout = Layout { columns: 1, page: 10, wrap: false };

    pub fn grid(columns: usize, visible_rows: usize) -> Layout {
        Layout { columns, page: columns * visible_rows.max(1), wrap: false }
    }
}

/// The movement for a key pressed in a group, ignoring keys with modifiers (those
/// belong to shortcuts).
pub fn group_key(event: &KeyDownEvent, axis: Axis) -> Option<Move> {
    let modifiers = event.keystroke.modifiers;
    if modifiers.control || modifiers.alt || modifiers.platform || modifiers.shift {
        return None;
    }
    Move::from_key(&event.keystroke.key, axis)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(_: usize) -> bool {
        true
    }

    #[test]
    fn wraps_in_menus_and_stops_in_lists() {
        assert_eq!(step(Some(2), 3, Move::Next, Layout::WRAPPING, all), Some(0));
        assert_eq!(step(Some(0), 3, Move::Prev, Layout::WRAPPING, all), Some(2));
        assert_eq!(step(Some(2), 3, Move::Next, Layout::LIST, all), Some(2));
        assert_eq!(step(Some(0), 3, Move::Prev, Layout::LIST, all), Some(0));
    }

    #[test]
    fn skips_disabled_items() {
        let enabled = |i| i != 1 && i != 2;
        assert_eq!(step(Some(0), 4, Move::Next, Layout::LIST, enabled), Some(3));
        assert_eq!(step(Some(3), 4, Move::Prev, Layout::LIST, enabled), Some(0));
        // Wrapping past disabled items at the end.
        let enabled = |i| i != 3;
        assert_eq!(step(Some(2), 4, Move::Next, Layout::WRAPPING, enabled), Some(0));
        // A disabled last item: the list stays put rather than landing on it.
        assert_eq!(step(Some(2), 4, Move::Next, Layout::LIST, enabled), Some(2));
    }

    #[test]
    fn nothing_enabled_or_empty() {
        assert_eq!(step(Some(0), 3, Move::Next, Layout::LIST, |_| false), None);
        assert_eq!(step(None, 0, Move::Next, Layout::LIST, all), None);
    }

    #[test]
    fn first_move_without_a_current_item() {
        assert_eq!(step(None, 5, Move::Next, Layout::LIST, all), Some(0));
        assert_eq!(step(None, 5, Move::Down, Layout::LIST, all), Some(0));
        assert_eq!(step(None, 5, Move::Prev, Layout::LIST, all), Some(4));
        // A stale index past the end counts as none.
        assert_eq!(step(Some(9), 5, Move::Up, Layout::LIST, all), Some(4));
    }

    #[test]
    fn home_end_and_paging() {
        let enabled = |i| i != 0;
        assert_eq!(step(Some(3), 5, Move::First, Layout::LIST, enabled), Some(1));
        assert_eq!(step(Some(1), 5, Move::Last, Layout::LIST, enabled), Some(4));
        let layout = Layout { columns: 1, page: 3, wrap: false };
        assert_eq!(step(Some(1), 10, Move::PageDown, layout, all), Some(4));
        assert_eq!(step(Some(8), 10, Move::PageDown, layout, all), Some(9));
        assert_eq!(step(Some(1), 10, Move::PageUp, layout, all), Some(0));
    }

    #[test]
    fn grid_rows_and_columns() {
        // 3 columns, 8 items:
        // 0 1 2
        // 3 4 5
        // 6 7
        let grid = Layout::grid(3, 2);
        assert_eq!(step(Some(1), 8, Move::Down, grid, all), Some(4));
        assert_eq!(step(Some(4), 8, Move::Up, grid, all), Some(1));
        assert_eq!(step(Some(1), 8, Move::Up, grid, all), Some(1));
        // Down from the last full row into a short one, and past it.
        assert_eq!(step(Some(4), 8, Move::Down, grid, all), Some(7));
        assert_eq!(step(Some(5), 8, Move::Down, grid, all), Some(5));
        assert_eq!(step(Some(2), 8, Move::Next, grid, all), Some(3));
        assert_eq!(step(Some(0), 8, Move::PageDown, grid, all), Some(6));
    }

    #[test]
    fn keys_map_by_axis() {
        assert_eq!(Move::from_key("down", Axis::Horizontal), Some(Move::Next));
        assert_eq!(Move::from_key("up", Axis::Vertical), Some(Move::Prev));
        assert_eq!(Move::from_key("down", Axis::Grid), Some(Move::Down));
        assert_eq!(Move::from_key("home", Axis::Grid), Some(Move::First));
        assert_eq!(Move::from_key("a", Axis::Grid), None);
    }
}
