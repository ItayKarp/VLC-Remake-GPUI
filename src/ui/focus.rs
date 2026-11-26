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

use gpui::{
    BoxShadow, ClickEvent, InteractiveElement, KeyContext, KeyDownEvent, Pixels, Point, Styled,
    actions, px,
};

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

/// Key context of every control that handles Space, Enter, Escape or the arrows
/// itself, so player shortcuts on those keys don't take them while it has focus.
pub const CONTROL: &str = "Control";
/// Key context of controls that take typed characters (text inputs, and menus and
/// dropdowns with type-ahead), so letter and symbol shortcuts don't fire in them.
pub const TYPING: &str = "Typing";

/// A key context made of several identifiers, e.g. `TextInput Control Typing`.
pub fn context(identifiers: &[&'static str]) -> KeyContext {
    let mut context = KeyContext::default();
    for identifier in identifiers {
        context.add(*identifier);
    }
    context
}

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

/// The ring's width when it's drawn as an outline.
const OUTLINE_WIDTH: f32 = 2.;

/// Draws the keyboard focus ring as a 2px line just inside the element's edge, for elements
/// with no border for [`ring`] to recolor (rows, listings) or whose border is already the
/// focus color (S9.1.1). It takes no room, so focusing never shifts the layout, and being
/// inside the edge, a parent's clipping can't hide it.
pub fn outline_ring<E: InteractiveElement + Styled>(element: E, p: &Palette) -> E {
    let color = p.focus_ring;
    element.focus_visible(move |style| {
        style.shadow(vec![
            BoxShadow::new(px(0.), px(0.), color).spread_radius(px(OUTLINE_WIDTH)).inset(),
        ])
    })
}

/// Where a menu opened from the keyboard goes: under the element Enter or Space pressed.
/// `None` for mouse and touch clicks, which controls that open on mouse-down handle there,
/// so their `on_click` only adds the keyboard's way in (S9.1.1).
pub fn keyboard_anchor(event: &ClickEvent) -> Option<Point<Pixels>> {
    match event {
        ClickEvent::Keyboard(keyboard) => Some(keyboard.bounds.bottom_left()),
        ClickEvent::Mouse(_) | ClickEvent::Touch(_) => None,
    }
}

/// A movement inside a group, from an arrow key or Home/End.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    /// The previous item (Left, or Up outside grids).
    Prev,
    /// The next item (Right, or Down outside grids).
    Next,
    /// The item one row up in a grid.
    Up,
    /// The item one row down in a grid.
    Down,
    /// The first item (Home).
    First,
    /// The last item (End).
    Last,
    /// A page back (PageUp).
    PageUp,
    /// A page forward (PageDown).
    PageDown,
}

impl Move {
    /// The movement for a key in a group laid out along `axis`. Groups on one axis
    /// accept both arrow pairs (Left/Up go back), like native radio groups.
    pub fn from_key(key: &str, axis: Axis) -> Option<Move> {
        Some(match (key, axis) {
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

/// The direction a group's items are laid out in, which decides what the arrow keys do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// A row: Left/Right (and Up/Down) step through it.
    Horizontal,
    /// A column: Up/Down (and Left/Right) step through it.
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
    let (mut index, direction) = jump(current, len, movement, layout);
    let paging = matches!(movement, Move::PageUp | Move::PageDown);
    for _ in 0..=len {
        let in_range = (0..len as isize).contains(&index);
        if !in_range && paging {
            // Paging past an end lands on it.
            return Some(if index < 0 { first_enabled } else { last_enabled });
        }
        if !in_range && !layout.wrap {
            return Some(current);
        }
        if !in_range {
            index = index.rem_euclid(len as isize);
        }
        if enabled(index as usize) {
            return Some(index as usize);
        }
        index += direction;
    }
    Some(current)
}

/// The first index a movement from `current` tries, and the direction to walk on in
/// when that item is disabled. Home and End walk inwards from their end, so they land
/// on the first or last enabled item.
fn jump(current: usize, len: usize, movement: Move, layout: Layout) -> (isize, isize) {
    let current = current as isize;
    let columns = layout.columns.max(1) as isize;
    let page = layout.page.max(1) as isize;
    match movement {
        Move::First => (0, 1),
        Move::Last => (len as isize - 1, -1),
        Move::Prev => (current - 1, -1),
        Move::Next => (current + 1, 1),
        Move::Up => (current - columns, -columns),
        Move::Down => (current + columns, columns),
        Move::PageUp => (current - page, -1),
        Move::PageDown => (current + page, 1),
    }
}

/// How a group is laid out, for [`step`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    /// Items per row (1 for lists).
    pub columns: usize,
    /// How many items PageUp/PageDown skip.
    pub page: usize,
    /// Whether moves cycle past the ends instead of stopping there.
    pub wrap: bool,
}

impl Layout {
    /// A one-dimensional group that wraps around (menus, swatches, tabs).
    pub const WRAPPING: Layout = Layout { columns: 1, page: 10, wrap: true };
    /// A one-dimensional group that stops at the ends (lists, dropdowns).
    pub const LIST: Layout = Layout { columns: 1, page: 10, wrap: false };

    /// A grid of `columns` items per row, where a page is the rows that are visible.
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
