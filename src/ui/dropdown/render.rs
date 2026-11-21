//! Drawing a [`Dropdown`]: the trigger face and, while open, the option list.

use gpui::{
    AnyElement, Bounds, Context, Div, MouseDownEvent, Pixels, Stateful, Window, accesskit, div,
    prelude::*, px,
};

use super::{Dropdown, VISIBLE_ROWS};
use crate::assets::icons;
use crate::theme::{self, Palette};
use crate::ui::focus;
use crate::ui::icon::icon;
use crate::ui::metrics;
use crate::ui::option_row::OptionRow;
use crate::ui::popover::{self, Side};

/// Height of one option row in the list.
const ROW_HEIGHT: f32 = 40.;
/// Opacity of a disabled trigger.
const DISABLED_OPACITY: f32 = 0.4;

impl Render for Dropdown {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let trigger = focus::ring(self.trigger_element(&p, cx), &p);
        let list = self
            .open
            .then(|| self.trigger.bounds())
            .flatten()
            .map(|bounds| self.list_element(bounds, &p, window, cx));
        div().w_full().child(trigger).children(list)
    }
}

impl Dropdown {
    /// The closed control face: the value and a chevron.
    fn trigger_element(&self, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let value = self.options.get(self.selected).cloned().unwrap_or_default();
        let hover = p.button_hover;
        let chevron = if self.open { icons::CHEVRON_UP } else { icons::CHEVRON_DOWN };
        div()
            .id("dropdown")
            .relative()
            .h(px(metrics::FIELD_HEIGHT))
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .pl(px(14.))
            .pr(px(12.))
            .rounded(px(metrics::RADIUS + 2.))
            .bg(p.control)
            .border_2()
            .border_color(if self.open { p.focus_ring } else { p.well })
            .text_size(px(metrics::TEXT))
            .text_color(p.text_muted)
            .track_focus(&self.focus_handle)
            .role(accesskit::Role::ComboBox)
            .aria_label(self.label.clone())
            .aria_expanded(self.open)
            .aria_value(value.clone())
            .on_key_down(cx.listener(Self::on_key_down))
            .child(self.trigger.element())
            .child(div().min_w_0().truncate().child(value))
            .child(icon(chevron, 20., p.icon))
            .when(!self.disabled, |t| {
                t.cursor_pointer()
                    .hover(move |s| s.bg(hover))
                    .on_click(cx.listener(|this, _, window, cx| this.toggle(window, cx)))
            })
            .when(self.disabled, |t| t.opacity(DISABLED_OPACITY))
    }

    /// The open list, attached below (or above) the trigger at `bounds`.
    fn list_element(
        &self,
        bounds: Bounds<Pixels>,
        p: &Palette,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let list_height =
            (ROW_HEIGHT * VISIBLE_ROWS as f32 + 12.).min(f32::from(popover::max_height(window)));
        let list = popover::surface(p, window)
            .id("dropdown-list")
            .relative()
            .w(bounds.size.width)
            .max_h(px(list_height))
            .p(px(6.))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .role(accesskit::Role::ListBox)
            .on_mouse_down_out(cx.listener(Self::on_mouse_down_outside))
            .child(self.list.element())
            .children(self.option_rows(cx));
        popover::attached(bounds, Side::Below, 4., 1, list)
    }

    fn option_rows(&self, cx: &mut Context<Self>) -> Vec<Stateful<Div>> {
        self.options
            .iter()
            .enumerate()
            .map(|(i, option)| {
                let row = OptionRow::new(i, option.clone())
                    .height(ROW_HEIGHT)
                    .checked(i == self.selected)
                    .highlighted(self.highlighted == Some(i));
                div()
                    .id(("option", i))
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        this.hover_option(i, *hovered, cx);
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| this.choose(i, cx)))
                    .child(row)
            })
            .collect()
    }

    /// Highlights the option under the mouse.
    fn hover_option(&mut self, index: usize, hovered: bool, cx: &mut Context<Self>) {
        if hovered && self.highlighted != Some(index) {
            self.highlighted = Some(index);
            cx.notify();
        }
    }

    /// Closes on a click outside the list, except on the trigger, which toggles on
    /// its own click.
    fn on_mouse_down_outside(
        &mut self,
        event: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.trigger.contains(&event.position) {
            self.close(cx);
        }
    }
}
