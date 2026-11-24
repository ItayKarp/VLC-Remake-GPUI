//! The Customize Player palette (`player_customize_01.png`): over the video, which it dims, and
//! not over the bars, which stay live. It lists what can be added (Spacer and Divider always,
//! each item while it isn't placed); dropping a bar piece on it removes the piece. Cancel and
//! Reset to default sit at its start, Save at its end. The list button by ✕ shows the layout
//! as a list instead (`list.rs`), for the keyboard.

use gpui::{
    AnyElement, ClickEvent, Context, Div, DragMoveEvent, KeyDownEvent, Stateful, div, prelude::*,
    px,
};

use super::outline::DraggedUnit;
use super::units;
use crate::assets::icons;
use crate::main_window::{MainWindow, TITLE_BAR_HEIGHT};
use crate::player_layout::Unit;
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::button::{Button, ButtonStyle, IconButton};
use crate::ui::icon::icon;
use crate::ui::metrics;

/// The palette's width in the design, when the window has room.
const WIDTH: f32 = 1048.;
/// Room kept between the palette and the bars and window edges.
const MARGIN: f32 = 24.;
/// Its padding.
const PADDING: f32 = 20.;
/// One unit's cell: its icon box over its name, which may take two lines.
const CELL_WIDTH: f32 = 110.;
const CELL_HEIGHT: f32 = 116.;
/// The dashed box around a unit's icon, and the icon in it.
const ICON_BOX: f32 = 32.;
const ICON: f32 = 20.;
/// The help line under the title.
const HELP: &str = "Drag items around and out of the box below to re-order them. Drag items \
                    into the box to remove them from the player.";
/// The help line while the palette shows the list.
const LIST_HELP: &str = "Use Tab to reach an item's buttons, and Enter or Space to move, remove \
                         or add it.";

impl MainWindow {
    /// The palette over the content area, while customizing; `title_bar` says whether the
    /// title bar shows above it.
    pub(in crate::main_window) fn render_customize_palette(
        &self,
        title_bar: bool,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        let top = if title_bar { TITLE_BAR_HEIGHT } else { 0. };
        let panel = self.render_palette_panel(p, cx)?;
        let area = div()
            .id("customize-area")
            .absolute()
            .top(px(top))
            .bottom(px(self.control_bar_height()))
            .left_0()
            .right_0()
            .flex()
            .flex_col()
            .bg(p.scrim)
            .occlude()
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .p(px(MARGIN))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(panel),
            )
            .children(self.render_new_row(p, cx));
        Some(area)
    }

    /// The palette itself: the title, the help line, the units and the buttons. It takes the
    /// pieces dropped on it out of the bars, and Escape cancels.
    fn render_palette_panel(&self, p: &Palette, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let customizing = self.customizing.as_ref()?;
        let panel = div()
            .id("customize-palette")
            .debug_selector(|| "customize-palette".into())
            .track_focus(&customizing.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    this.cancel_customizing(window, cx);
                    cx.stop_propagation();
                }
            }))
            .w(px(WIDTH))
            .max_w_full()
            .max_h_full()
            .flex()
            .flex_col()
            .rounded(px(metrics::RADIUS_LARGE))
            .bg(p.panel)
            .text_color(p.text)
            .shadow_lg()
            .on_drag_move(cx.listener(|this, event: &DragMoveEvent<DraggedUnit>, _, cx| {
                this.track_drop_area(None, event.bounds.contains(&event.event.position), cx);
            }))
            .on_drop(
                cx.listener(|this, dragged: &DraggedUnit, _, cx| this.remove_unit(dragged, cx)),
            )
            .child(header(customizing.as_list, p, cx))
            .child(help(customizing.as_list, p))
            .child(if customizing.as_list {
                self.render_layout_list(p, cx)
            } else {
                self.render_units(p)
            })
            .child(palette_footer(customizing.has_changes(), cx));
        Some(panel)
    }

    /// The units the palette offers, in a grid that scrolls in a short window.
    fn render_units(&self, p: &Palette) -> Stateful<Div> {
        let units = self.shown_layout().palette();
        let cells = units.into_iter().enumerate().map(|(index, unit)| cell(index, unit, p));
        div()
            .id("customize-units")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(PADDING))
            .py(px(16.))
            .flex()
            .flex_row()
            .flex_wrap()
            .children(cells)
    }
}

/// Cancel and Reset to default at the start, Save (once there's a change) at the end.
fn palette_footer(changed: bool, cx: &mut Context<MainWindow>) -> Div {
    let cancel = Button::new("customize-cancel", tr!("Cancel"))
        .style(ButtonStyle::Filled)
        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
            this.cancel_customizing(window, cx);
        }));
    let reset = Button::new("customize-reset", tr!("Reset to default"))
        .style(ButtonStyle::Ghost)
        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.reset_layout(cx)));
    let save = Button::new("customize-save", tr!("Save"))
        .primary()
        .disabled(!changed)
        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.save_layout(window, cx)));
    div()
        .px(px(PADDING))
        .py(px(16.))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(div().flex().flex_row().gap(px(8.)).child(cancel).child(reset))
        .child(save)
}

/// The help line under the title.
fn help(as_list: bool, p: &Palette) -> Div {
    let help = if as_list { LIST_HELP } else { HELP };
    div().px(px(PADDING)).py(px(12.)).bg(p.sunken).text_size(text_px(13.)).child(help)
}

/// The title, the list button and the ✕ that cancels.
fn header(as_list: bool, p: &Palette, cx: &mut Context<MainWindow>) -> Div {
    let list = IconButton::new("customize-as-list", icons::LIST, tr!("Edit as a list"))
        .size(28., 16.)
        .selected(as_list)
        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_list(cx)));
    let close =
        IconButton::new("customize-close", icons::CLOSE, tr!("Cancel")).size(28., 16.).on_click(
            cx.listener(|this, _: &ClickEvent, window, cx| this.cancel_customizing(window, cx)),
        );
    div()
        .px(px(PADDING))
        .pt(px(12.))
        .pb(px(8.))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(
            div().text_size(text_px(14.)).text_color(p.text_strong).child(tr!("Customize Player")),
        )
        .child(div().flex().flex_row().gap(px(4.)).child(list).child(close))
}

/// One unit of the palette, dragged out to add it.
fn cell(index: usize, unit: Unit, p: &Palette) -> AnyElement {
    let hover = p.hover;
    div()
        .id(("palette-unit", index))
        .debug_selector(move || format!("palette-{}", units::english(unit)))
        .w(px(CELL_WIDTH))
        .h(px(CELL_HEIGHT))
        .pt(px(16.))
        .px(px(4.))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(8.))
        .rounded(px(metrics::RADIUS))
        .cursor_grab()
        .hover(move |style| style.bg(hover))
        .on_drag(DraggedUnit { unit, from: None }, |dragged, _, _, cx| dragged.preview(cx))
        .child(icon_box(unit, p))
        .child(
            div()
                .text_size(text_px(13.))
                .text_center()
                .text_color(p.text)
                .child(units::label(unit)),
        )
        .into_any_element()
}

/// The unit's icon in a dashed box: Spacer's is empty, Divider's a line.
fn icon_box(unit: Unit, p: &Palette) -> Div {
    let frame = div()
        .size(px(ICON_BOX))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_dashed()
        .border_color(p.text_muted);
    match (unit, units::icon(unit)) {
        (_, Some(path)) => frame.child(icon(path, ICON, p.icon)),
        (Unit::Divider, None) => frame.child(div().w(px(1.)).h(px(ICON)).bg(p.icon)),
        _ => frame,
    }
}
