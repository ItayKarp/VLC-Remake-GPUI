//! Drawing a [`PresetPicker`]: the line naming the preset being edited, the dropdown face, the
//! list it opens upward, and the status line; and its keys.

use gpui::{
    AnyElement, Bounds, ClickEvent, Context, Div, KeyDownEvent, MouseDownEvent, Pixels,
    SharedString, Stateful, Window, accesskit, div, prelude::*, px,
};

use super::{PresetPicker, Row};
use crate::assets::icons;
use crate::i18n::DirectionalStyle;
use crate::settings::presets::{self, names};
use crate::theme::text_px;
use crate::theme::{self, Palette};
use crate::ui::focus::{self, Layout, Move};
use crate::ui::icon::icon;
use crate::ui::metrics;
use crate::ui::popover::{self, Side};

/// The height of a row in the list.
const ROW_HEIGHT: f32 = 40.;
/// The size of the ✓ at a row's end.
const CHECK_SIZE: f32 = 22.;
/// The size of the clickable square around a row's `···`.
const MORE_SIZE: f32 = 28.;
/// The layers the open list and a preset's menu are painted on, above the window's own.
const LIST_PRIORITY: usize = 2;
const MENU_PRIORITY: usize = 3;

impl Render for PresetPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let active = presets::active(cx).map(SharedString::from);
        let note = active.clone().map(|name| {
            let text = tr!("Changes on these pages are saved to “{name}”.", name);
            div().text_size(text_px(metrics::TEXT_SMALL)).text_color(p.text_muted).child(text)
        });
        let status = self.status.clone().map(|status| {
            let color = if status.failed { p.danger } else { p.text_muted };
            div().text_size(text_px(metrics::TEXT_SMALL)).text_color(color).child(status.text)
        });
        let list = self
            .open
            .then(|| self.trigger.bounds())
            .flatten()
            .map(|bounds| self.list_element(bounds, &p, window, cx));
        let menu = self.menu.as_ref().map(|(position, menu, _)| {
            popover::at_position(*position, MENU_PRIORITY, menu.clone())
        });
        let label = active.unwrap_or_else(|| crate::i18n::text(names::DEFAULT).into());
        div()
            .w_full()
            .flex_row_start()
            .flex_col()
            .gap(px(6.))
            .children(note)
            .child(focus::ring(self.trigger_element(label, &p, cx), &p))
            .children(status)
            .children(list)
            .children(menu)
    }
}

impl PresetPicker {
    /// The closed face: the preset in use and a chevron.
    fn trigger_element(
        &self,
        label: SharedString,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let hover = p.button_hover;
        let chevron = if self.open { icons::CHEVRON_DOWN } else { icons::CHEVRON_UP };
        div()
            .id("preset-picker")
            .relative()
            .h(px(metrics::FIELD_HEIGHT))
            .w_full()
            .flex_row_start()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .ps(px(14.))
            .pe(px(12.))
            .rounded(px(metrics::RADIUS + 2.))
            .bg(p.control)
            .border_2()
            .border_color(if self.open { p.focus_ring } else { p.well })
            .text_size(text_px(metrics::TEXT))
            .text_color(p.text_muted)
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .track_focus(&self.focus_handle)
            .key_context(focus::context(&["PresetPicker", focus::CONTROL]))
            .role(accesskit::Role::ComboBox)
            .aria_label(tr!("Setting preset"))
            .aria_expanded(self.open)
            .aria_value(label.clone())
            .on_key_down(cx.listener(Self::on_key_down))
            .on_click(cx.listener(|this, _, window, cx| this.toggle(window, cx)))
            .child(self.trigger.element())
            .child(div().min_w_0().truncate().child(label))
            .child(icon(chevron, 20., p.icon))
    }

    /// The open list, above the trigger at `bounds` (below when there's no room).
    fn list_element(
        &self,
        bounds: Bounds<Pixels>,
        p: &Palette,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let rows: Vec<_> = Self::rows(cx)
            .into_iter()
            .enumerate()
            .map(|(i, row)| self.row(i, row, p, cx))
            .collect();
        let list = popover::surface(p, window)
            .id("preset-list")
            .relative()
            .w(bounds.size.width)
            .p(px(6.))
            .overflow_y_scroll()
            .role(accesskit::Role::ListBox)
            .on_mouse_down_out(cx.listener(Self::on_mouse_down_outside))
            .child(self.list.element())
            .children(rows);
        popover::attached(bounds, Side::AboveEnd, 4., LIST_PRIORITY, list)
    }

    /// Row `index` of the list.
    fn row(&self, index: usize, row: Row, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let active = presets::active(cx).map(SharedString::from);
        let checked = match &row {
            Row::Custom(name) => active.as_ref() == Some(name),
            Row::Default => active.is_none(),
            Row::New | Row::Import => false,
        };
        let hover = p.hover;
        let face = div()
            .id(("preset-row", index))
            .h(px(ROW_HEIGHT))
            .flex_row_start()
            .items_center()
            .gap(px(10.))
            .px(px(10.))
            .rounded(px(metrics::RADIUS + 2.))
            .text_size(text_px(metrics::TEXT))
            .text_color(if checked { p.text_strong } else { p.text })
            .role(accesskit::Role::ListBoxOption)
            .aria_selected(checked)
            .when(self.highlighted == Some(index) || checked, |r| r.bg(hover))
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if *hovered && this.highlighted != Some(index) {
                    this.highlighted = Some(index);
                    cx.notify();
                }
            }));
        let face = self.row_content(face, &row, p, cx);
        let face = face.when(checked, |r| r.child(icon(icons::CHECK, CHECK_SIZE, p.accent_fg)));
        if self.renaming_row(&row) {
            return face;
        }
        face.on_click(cx.listener(move |this, _, window, cx| this.activate(&row, window, cx)))
    }

    /// The row's icon or `···` and its label (or the name being edited).
    fn row_content(
        &self,
        face: Stateful<Div>,
        row: &Row,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let label = |text: SharedString| div().flex_1().min_w_0().truncate().child(text);
        match row {
            Row::New => face
                .child(slot(icon(icons::ADD, metrics::ICON, p.icon)))
                .child(label(tr!("New Preset").into())),
            Row::Import => face
                .child(slot(icon(icons::FILE, metrics::ICON, p.icon)))
                .child(label(tr!("Import Preset…").into())),
            Row::Default => {
                face.child(slot(div())).child(label(crate::i18n::text(names::DEFAULT).into()))
            }
            Row::Custom(name) => {
                let face = face.child(Self::more_button(name.clone(), p, cx));
                match self.renaming.as_ref().filter(|renaming| &renaming.name == name) {
                    Some(renaming) => face.child(div().flex_1().child(renaming.input.clone())),
                    None => face.child(label(name.clone())),
                }
            }
        }
    }

    /// The `···` that opens preset `name`'s menu.
    fn more_button(name: SharedString, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let hover = p.button_hover;
        div()
            .id(SharedString::from(format!("preset-more-{name}")))
            .size(px(MORE_SIZE))
            .flex_none()
            .flex_row_start()
            .items_center()
            .justify_center()
            .rounded(px(metrics::RADIUS))
            .hover(move |s| s.bg(hover))
            .aria_label(SharedString::from(tr!("More for {name}", name)))
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                cx.stop_propagation();
                this.open_menu(&name, event.position(), window, cx);
            }))
            .child(icon(icons::MORE, metrics::ICON, p.icon))
    }

    fn renaming_row(&self, row: &Row) -> bool {
        matches!((row, &self.renaming), (Row::Custom(name), Some(renaming)) if *name == renaming.name)
    }

    /// Closes on a click outside the list, except on the trigger, which toggles on its own
    /// click, and inside a preset's menu.
    fn on_mouse_down_outside(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let in_menu = self
            .menu
            .as_ref()
            .is_some_and(|(_, menu, _)| menu.read(cx).contains(event.position, cx));
        if in_menu || self.trigger.contains(event.position) {
            return;
        }
        self.finish_renaming(true, window, cx);
        self.close(cx);
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        if self.renaming.is_some() {
            if key == "escape" {
                self.finish_renaming(false, window, cx);
                cx.stop_propagation();
            }
            return;
        }
        if !self.open {
            if matches!(key, "enter" | "space" | "up") {
                self.toggle(window, cx);
                cx.stop_propagation();
            }
            return;
        }
        let rows = Self::rows(cx);
        let movement = match key {
            "up" => Some(Move::Prev),
            "down" => Some(Move::Next),
            "home" => Some(Move::First),
            "end" => Some(Move::Last),
            _ => None,
        };
        if let Some(movement) = movement {
            self.highlighted =
                focus::step(self.highlighted, rows.len(), movement, Layout::WRAPPING, |_| true);
            cx.notify();
        } else {
            self.on_list_key(key, &rows, window, cx);
        }
        cx.stop_propagation();
    }

    /// Enter, Space, Right, the menu key and Escape while the list is open.
    fn on_list_key(
        &mut self,
        key: &str,
        rows: &[Row],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let row = self.highlighted.and_then(|index| rows.get(index)).cloned();
        match (key, row) {
            ("enter" | "space", Some(row)) => self.activate(&row, window, cx),
            ("right" | "menu", Some(Row::Custom(name))) => {
                let position = self.list.bounds().map_or_default(|bounds| bounds.center());
                self.open_menu(&name, position, window, cx);
                if let Some((_, menu, _)) = &self.menu {
                    menu.update(cx, crate::ui::menu::ContextMenu::highlight_first);
                }
            }
            ("escape" | "tab", _) => self.close(cx),
            _ => {}
        }
    }
}

/// A fixed-width slot at the start of a row, so every label lines up.
fn slot(content: impl IntoElement) -> Div {
    div()
        .size(px(MORE_SIZE))
        .flex_none()
        .flex_row_start()
        .items_center()
        .justify_center()
        .child(content)
}
