//! Drawing a [`ContextMenu`]: its rows, the menu surface and the open submenu.

use gpui::{
    AnyElement, Context, DismissEvent, Div, MouseButton, MouseDownEvent, SharedString, Stateful,
    Window, accesskit, div, prelude::*, px,
};

use super::{ContextMenu, MenuItem};
use crate::theme::{self, Palette};
use crate::ui::focus;
use crate::ui::metrics;
use crate::ui::option_row::OptionRow;
use crate::ui::popover::{self, Side};
use crate::ui::tooltip;

/// Height of one menu row.
const ROW_HEIGHT: f32 = 40.;
/// Widest a menu gets, in logical pixels.
const MAX_WIDTH: f32 = 420.;

impl Render for ContextMenu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let submenu = self.submenu.as_ref().and_then(|open| {
            let bounds = self.rows[open.index].bounds()?;
            Some(popover::attached(bounds, Side::Right, 4., self.depth + 2, open.menu.clone()))
        });
        // The submenu renders inside this element, so keys pressed in it pass
        // through here: Left and Escape are caught on the way down.
        div()
            .relative()
            .track_focus(&self.focus_handle)
            .key_context(focus::context(&["Menu", focus::CONTROL, focus::TYPING]))
            .on_key_down(cx.listener(Self::on_key_down))
            .capture_key_down(cx.listener(Self::on_key_down_capture_left))
            .child(self.surface(&p, window, cx))
            .children(submenu)
    }
}

impl ContextMenu {
    /// The menu panel holding the rows.
    fn surface(&self, p: &Palette, window: &Window, cx: &mut Context<Self>) -> Stateful<Div> {
        popover::surface(p, window)
            .id("menu")
            .role(accesskit::Role::Menu)
            // Only the root menu closes on outside clicks; it knows about the
            // whole chain of submenus.
            .when(self.depth == 0, |menu| {
                menu.on_mouse_down_out(cx.listener(Self::on_mouse_down_outside))
            })
            // Don't let a right click inside reopen the menu underneath.
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .min_w(px(self.min_width))
            .max_w(px(MAX_WIDTH))
            .p(px(6.))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .text_size(px(metrics::TEXT))
            .child(self.tracker.element())
            .children(self.row_elements(p, cx))
    }

    fn row_elements(&self, p: &Palette, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let any_icons = self.items.iter().any(|item| {
            matches!(
                item,
                MenuItem::Action { icon: Some(_), .. } | MenuItem::Submenu { icon: Some(_), .. }
            )
        });
        (0..self.items.len()).map(|i| self.row_element(i, any_icons, p, cx)).collect()
    }

    /// The row for the item at `i`, wired for hover and click when selectable.
    fn row_element(
        &self,
        i: usize,
        any_icons: bool,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let item = &self.items[i];
        let submenu_open = self.submenu.as_ref().is_some_and(|s| s.index == i);
        let Some(row) = option_row(item, submenu_open) else {
            return separator(i, p);
        };
        let row = row
            .highlighted(self.highlighted == Some(i) || submenu_open)
            .reserve_icon(any_icons)
            .height(ROW_HEIGHT);
        let selectable = item.selectable() || submenu_open;
        let reason = item.hover_text().filter(|_| !submenu_open).cloned();
        div()
            .id(i)
            .relative()
            .child(self.rows[i].element())
            .child(row)
            .when_some(reason, |r, reason| r.tooltip(tooltip::text(reason, None::<SharedString>)))
            .when(selectable, |r| {
                r.on_hover(cx.listener(move |this, hovered: &bool, window, cx| {
                    this.hover_row(i, *hovered, window, cx);
                }))
                .on_click(cx.listener(move |this, _, window, cx| this.activate(i, window, cx)))
            })
            .into_any_element()
    }

    /// Highlights the row under the mouse, opening its submenu or closing another.
    fn hover_row(&mut self, i: usize, hovered: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !hovered {
            return;
        }
        self.highlighted = Some(i);
        if self.items[i].is_submenu() {
            self.open_submenu(i, false, window, cx);
        } else {
            self.close_submenu(window, cx);
        }
        cx.notify();
    }

    /// Closes the whole menu chain on a click outside all of it.
    fn on_mouse_down_outside(
        &mut self,
        event: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.contains(event.position, cx) {
            cx.emit(DismissEvent);
        }
    }
}

/// The option row for `item`, or `None` for a separator. An open submenu stays
/// enabled while its items are lent to the submenu.
fn option_row(item: &MenuItem, submenu_open: bool) -> Option<OptionRow> {
    let row = match item {
        MenuItem::Separator => return None,
        MenuItem::Action { label, icon, shortcut, disabled, .. } => {
            OptionRow::new("row", label.clone())
                .icon(*icon)
                .shortcut(shortcut.clone())
                .disabled(*disabled)
                .check_column(false)
        }
        MenuItem::Check { label, icon, checked, disabled, .. } => {
            OptionRow::new("row", label.clone()).icon(*icon).checked(*checked).disabled(*disabled)
        }
        MenuItem::Submenu { label, icon, items, .. } => OptionRow::new("row", label.clone())
            .icon(*icon)
            .submenu(true)
            .disabled(items.is_empty() && !submenu_open),
    };
    Some(row)
}

fn separator(i: usize, p: &Palette) -> AnyElement {
    div().id(i).my(px(4.)).mx(px(8.)).h(px(1.)).bg(p.divider).into_any_element()
}
