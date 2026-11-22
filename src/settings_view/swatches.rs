//! A row of color swatches, one of them chosen (`global_01.png`'s Accent color): a well of
//! squares, the chosen one on a lighter tile. A radio group: one tab stop, arrow keys move.

use std::rc::Rc;

use gpui::{Div, ElementId, Hsla, SharedString, Stateful, accesskit, div, prelude::*, px};

use crate::settings::registry::Choice;
use crate::theme::Palette;
use crate::ui::focus::{self, Axis, Layout};
use crate::ui::metrics;

/// A swatch's square, and the tile around it.
const SQUARE: f32 = 18.;
const TILE: f32 = 30.;

/// Called with the index of the swatch picked.
pub(super) type SelectHandler = Rc<dyn Fn(&usize, &mut gpui::Window, &mut gpui::App)>;

/// What the swatch row shows.
pub(super) struct SwatchRow {
    /// Names the group for screen readers.
    pub label: &'static str,
    /// The swatches.
    pub choices: &'static [Choice],
    /// The chosen swatch's index.
    pub selected: usize,
    /// Each swatch's color by choice id.
    pub color: fn(&str) -> Option<Hsla>,
}

/// The swatch row; without `on_select` it ignores clicks and keys (a disabled row).
pub(super) fn swatches(
    id: impl Into<ElementId>,
    row: &SwatchRow,
    p: &Palette,
    on_select: Option<&SelectHandler>,
) -> Stateful<Div> {
    let (selected, len) = (row.selected, row.choices.len());
    let group = div()
        .id(id)
        .w_full()
        .flex()
        .justify_between()
        .gap(px(metrics::WELL_INSET))
        .p(px(metrics::WELL_INSET))
        .rounded(px(metrics::RADIUS + 2.))
        .bg(p.well)
        .border_2()
        .border_color(p.well)
        .tab_index(0)
        .role(accesskit::Role::RadioGroup)
        .aria_label(SharedString::from(row.label));
    let group = match on_select.cloned() {
        Some(on_select) => {
            group.key_context(focus::CONTROL).on_key_down(move |event, window, cx| {
                let Some(movement) = focus::group_key(event, Axis::Horizontal) else { return };
                let Some(next) = focus::step(Some(selected), len, movement, Layout::LIST, |_| true)
                else {
                    return;
                };
                cx.stop_propagation();
                if next != selected {
                    on_select(&next, window, cx);
                }
            })
        }
        None => group,
    };
    let swatches = row.choices.iter().enumerate().map(|(index, choice)| {
        let tile = Tile { index, choice: *choice, chosen: index == selected };
        swatch(&tile, (row.color)(choice.id), p, on_select.cloned())
    });
    focus::ring(group, p).children(swatches)
}

/// One swatch's place in the row.
struct Tile {
    index: usize,
    choice: Choice,
    chosen: bool,
}

/// One swatch: a colored square on a tile that shows when it's chosen.
fn swatch(
    tile: &Tile,
    color: Option<Hsla>,
    p: &Palette,
    on_select: Option<SelectHandler>,
) -> Stateful<Div> {
    let Tile { index, choice, chosen } = *tile;
    let hover = p.button_hover;
    div()
        .id(index)
        .size(px(TILE))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(metrics::RADIUS))
        .when(chosen, |tile| tile.bg(p.accent_tint))
        .when(!chosen, |tile| tile.cursor_pointer().hover(move |s| s.bg(hover)))
        .role(accesskit::Role::RadioButton)
        .aria_label(SharedString::from(choice.label))
        .aria_selected(chosen)
        .when_some(on_select, |tile, on_select| {
            tile.on_click(move |_, window, cx| on_select(&index, window, cx))
        })
        .child(div().size(px(SQUARE)).rounded(px(2.)).when_some(color, Styled::bg))
}
