//! Customize Player as a list, for editing the layout without a pointer (S8.7 edge cases): the
//! palette's list button shows it in place of the units. Each bar is a heading over its units
//! in order, each with Move up, Move down and Remove buttons reached with Tab and pressed with
//! Enter or Space; moving off a bar's end goes into the next bar. Under the bars, what can be
//! added, each with an Add button that puts it at the end of the control bar.
//!
//! The buttons are named after their unit, so the one pressed keeps focus as its unit moves.

use std::collections::HashMap;

use gpui::{AnyElement, ClickEvent, Context, Div, Stateful, Window, div, prelude::*, px};

use super::units;
use crate::assets::icons;
use crate::main_window::MainWindow;
use crate::player_layout::{Item, Layout, Spot, Step, Target, Unit, Zone};
use crate::theme::Palette;
use crate::ui::button::IconButton;

/// A list row's height and its buttons' size.
const ROW_HEIGHT: f32 = 32.;
const BUTTON: f32 = 28.;
const BUTTON_ICON: f32 = 16.;
/// The list's padding.
const PADDING: f32 = 20.;

impl MainWindow {
    /// The layout as a list, then what can be added.
    pub(super) fn render_layout_list(&self, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let layout = self.shown_layout();
        let mut rows = Vec::new();
        let mut keys = Keys::default();
        for zone in layout.zones() {
            rows.push(heading(zone_name(layout, zone), p));
            rows.extend(zone_rows(layout, zone, &mut keys, p, cx));
        }
        rows.push(heading("Add to the control bar".into(), p));
        let adds = layout.palette().into_iter().map(|unit| add_row(unit, p, cx));
        rows.extend(adds);
        div()
            .id("customize-list")
            .debug_selector(|| "customize-list".into())
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(PADDING))
            .py(px(12.))
            .flex()
            .flex_col()
            .children(rows)
    }

    /// Moves the unit at `spot` a place `step`.
    pub(super) fn nudge_unit(&mut self, spot: Spot, step: Step, cx: &mut Context<Self>) {
        let Some(customizing) = &mut self.customizing else { return };
        if customizing.draft.nudge(spot, step).is_some() {
            customizing.bounds.clear();
            cx.notify();
        }
    }

    /// Removes the unit at `spot`, keeping the keyboard in the palette.
    pub(super) fn remove_listed(
        &mut self,
        spot: Spot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(customizing) = &mut self.customizing else { return };
        customizing.draft.remove(spot);
        customizing.bounds.clear();
        customizing.focus.focus(window, cx);
        cx.notify();
    }

    /// Adds `unit` at the end of the control bar's last row, keeping the keyboard in the
    /// palette.
    pub(super) fn add_listed(&mut self, unit: Unit, window: &mut Window, cx: &mut Context<Self>) {
        let Some(customizing) = &mut self.customizing else { return };
        let zone = Zone::Row(customizing.draft.rows.len().saturating_sub(1));
        let index = customizing.draft.pieces(zone).len();
        customizing.draft.add(unit, Target::Into(Spot { zone, index }));
        customizing.bounds.clear();
        customizing.focus.focus(window, cx);
        cx.notify();
    }
}

/// Names for the units' buttons that follow them as they move: an item's own, a spacer's or
/// divider's its place among the spacers or dividers.
#[derive(Default)]
struct Keys {
    counts: HashMap<Unit, usize>,
}

impl Keys {
    fn next(&mut self, unit: Unit) -> String {
        let count = self.counts.entry(unit).or_default();
        *count += 1;
        format!("{unit:?}-{count}")
    }
}

/// `zone`'s units, each a row, and the window controls that stay put.
fn zone_rows(
    layout: &Layout,
    zone: Zone,
    keys: &mut Keys,
    p: &Palette,
    cx: &mut Context<MainWindow>,
) -> Vec<AnyElement> {
    let mut rows = Vec::new();
    let mut index = 0;
    while index < layout.pieces(zone).len() {
        let spot = Spot { zone, index };
        index += 1;
        match layout.unit_at(spot) {
            Some((unit, range)) => {
                let movable = |step| layout.clone().nudge(spot, step).is_some();
                let moves = [Step::Earlier, Step::Later].map(movable);
                let listed = Listed { spot, unit, key: keys.next(unit), moves };
                rows.push(unit_row(&listed, p, cx));
                index = range.end;
            }
            None => rows.extend(fixed_item(layout, spot).map(|item| fixed_row(item, p))),
        }
    }
    rows
}

/// A unit in the list: where it is, its buttons' name, and whether it can move up and down.
struct Listed {
    spot: Spot,
    unit: Unit,
    key: String,
    moves: [bool; 2],
}

/// The heading over `zone`'s units.
fn zone_name(layout: &Layout, zone: Zone) -> String {
    match zone {
        Zone::TitleBar => "Title bar".into(),
        Zone::WindowControls => "Window controls".into(),
        Zone::Row(_) if layout.rows.len() == 1 => "Control bar".into(),
        Zone::Row(0) => "Control bar, top row".into(),
        Zone::Row(_) => "Control bar, bottom row".into(),
    }
}

/// The window control at `spot` that stays put, if there's one.
fn fixed_item(layout: &Layout, spot: Spot) -> Option<Item> {
    match layout.pieces(spot.zone).get(spot.index)? {
        crate::player_layout::Piece::Item(item) if item.is_fixed() => Some(*item),
        _ => None,
    }
}

fn heading(name: String, p: &Palette) -> AnyElement {
    div()
        .flex_none()
        .pt(px(12.))
        .pb(px(4.))
        .text_size(px(12.))
        .text_color(p.text_muted)
        .child(name)
        .into_any_element()
}

/// A row: the unit's icon and name, then its buttons.
fn row(unit: Unit, buttons: Vec<AnyElement>, p: &Palette) -> AnyElement {
    let icon = units::icon(unit).map(|path| crate::ui::icon::icon(path, BUTTON_ICON, p.icon));
    div()
        .h(px(ROW_HEIGHT))
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .child(div().w(px(BUTTON_ICON)).flex_none().children(icon))
        .child(div().flex_1().text_size(px(13.)).text_color(p.text).child(units::label(unit)))
        .children(buttons)
        .into_any_element()
}

/// A unit in the bars, with Move up, Move down and Remove.
fn unit_row(listed: &Listed, p: &Palette, cx: &mut Context<MainWindow>) -> AnyElement {
    let Listed { spot, unit, ref key, moves } = *listed;
    let name = units::label(unit);
    let step = |step: Step, icon, label: &str, cx: &mut Context<MainWindow>| {
        let movable = moves[usize::from(step == Step::Later)];
        IconButton::new(format!("list-{key}-{step:?}"), icon, format!("Move {name} {label}"))
            .size(BUTTON, BUTTON_ICON)
            .disabled(!movable)
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.nudge_unit(spot, step, cx);
            }))
            .into_any_element()
    };
    let remove =
        IconButton::new(format!("list-{key}-remove"), icons::CLOSE, format!("Remove {name}"))
            .size(BUTTON, BUTTON_ICON)
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.remove_listed(spot, window, cx);
            }))
            .into_any_element();
    let buttons = vec![
        step(Step::Earlier, icons::ARROW_UP, "up", cx),
        step(Step::Later, icons::ARROW_DOWN, "down", cx),
        remove,
    ];
    row(unit, buttons, p)
}

/// A window control that stays put: no buttons, but a word on why.
fn fixed_row(item: Item, p: &Palette) -> AnyElement {
    let note = div().text_size(px(12.)).text_color(p.text_muted).child("Stays in place");
    row(Unit::Item(item), vec![note.into_any_element()], p)
}

/// What can be added, with Add.
fn add_row(unit: Unit, p: &Palette, cx: &mut Context<MainWindow>) -> AnyElement {
    let name = units::label(unit);
    let add = IconButton::new(format!("list-add-{unit:?}"), icons::ADD, format!("Add {name}"))
        .size(BUTTON, BUTTON_ICON)
        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
            this.add_listed(unit, window, cx);
        }))
        .into_any_element();
    row(unit, vec![add], p)
}
