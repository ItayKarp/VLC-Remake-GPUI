//! The bars in edit mode: every unit outlined with a dashed border and draggable, a marker
//! where a drop would land, and the place for a second row while a drag is on.

use gpui::{
    AnyElement, Context, Div, DragMoveEvent, ElementId, IntoElement, ParentElement, Render,
    SharedString, Styled, Window, div, prelude::*, px,
};

use super::units;
use crate::main_window::MainWindow;
use crate::main_window::control_bar::{CONTROL_BAR_HEIGHT, Place, divider};
use crate::player_layout::{Item, MAX_ROWS, Piece, Spot, Target, Unit, Zone};
use crate::theme::{self, Palette};
use crate::ui::icon::icon;
use crate::ui::metrics;

/// The smallest an outlined piece gets, so an empty one (a spacer, a name with nothing
/// playing) can still be picked up.
const MIN_PIECE: f32 = 28.;
/// The drop marker's width and height.
const MARKER_WIDTH: f32 = 2.;
const MARKER_HEIGHT: f32 = 24.;
/// Padding inside an outline.
const OUTLINE_PAD: f32 = 2.;
/// The drag preview's icon size.
const PREVIEW_ICON: f32 = 18.;

/// A unit being dragged: from the bars (`from`) or out of the palette.
#[derive(Clone, Debug)]
pub(in crate::main_window) struct DraggedUnit {
    /// What is dragged.
    pub(super) unit: Unit,
    /// Where it was in the bars; `None` from the palette.
    pub(super) from: Option<Spot>,
}

impl DraggedUnit {
    /// The view that follows the pointer.
    pub(super) fn preview(&self, cx: &mut gpui::App) -> gpui::Entity<UnitPreview> {
        let unit = self.unit;
        cx.new(|_| UnitPreview { unit })
    }
}

/// What follows the pointer while a unit is dragged: its icon and name.
pub(in crate::main_window) struct UnitPreview {
    unit: Unit,
}

impl Render for UnitPreview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .px(px(10.))
            .py(px(6.))
            .rounded(px(metrics::RADIUS))
            .bg(p.popover)
            .text_color(p.text)
            .text_size(px(13.))
            .shadow_md()
            .children(units::icon(self.unit).map(|path| icon(path, PREVIEW_ICON, p.icon)))
            .child(units::label(self.unit))
    }
}

impl MainWindow {
    /// The pieces of `zone` in edit mode, drawn as in the bar `place`: each unit outlined and
    /// draggable, with a marker where a drop would land.
    pub(in crate::main_window) fn render_edit_pieces(
        &self,
        zone: Zone,
        place: Place,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let layout = self.shown_layout();
        let pieces = layout.pieces(zone).to_vec();
        // A drop place left from a drag that ended elsewhere means nothing.
        let target = self.customizing.as_ref().and_then(|c| c.target);
        let target = target.filter(|_| cx.has_active_drag());
        let marker = |index| target == Some(Target::Into(Spot { zone, index }));
        let mut drawn = Vec::new();
        let mut index = 0;
        while index < pieces.len() {
            if marker(index) {
                drawn.push(drop_marker(p));
            }
            let spot = Spot { zone, index };
            let Some((unit, range)) = layout.unit_at(spot) else {
                drawn.push(self.render_piece(pieces[index], place, p, cx));
                index += 1;
                continue;
            };
            let inner = range.clone().map(|i| self.render_piece(pieces[i], place, p, cx)).collect();
            drawn.push(Self::outlined(spot, unit, inner, p, cx));
            index = range.end;
        }
        if marker(pieces.len()) {
            drawn.push(drop_marker(p));
        }
        drawn
    }

    /// A piece as the bar `place` draws it outside edit mode, with a name standing in for what
    /// shows nothing yet.
    fn render_piece(
        &self,
        piece: Piece,
        place: Place,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match piece {
            // Outside edit mode it's centered on the window; here it's in line, to be moved.
            Piece::Item(Item::Title) if place == Place::TitleBar => {
                placeholder(self.title(cx).into(), p)
            }
            Piece::Item(item) => self.render_item(item, place, p, cx),
            Piece::Spacer => div().flex_1().into_any_element(),
            Piece::Divider => divider(p).into_any_element(),
            Piece::Gap(width) => div().w(px(width)).flex_none().into_any_element(),
        }
    }

    /// `inner`, the unit at `spot`, in a dashed outline that drags it.
    fn outlined(
        spot: Spot,
        unit: Unit,
        inner: Vec<AnyElement>,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let grows = matches!(unit, Unit::Spacer | Unit::Item(Item::CurrentTime));
        let handle = div()
            .id(piece_id(spot))
            .debug_selector(move || format!("customize-{}", units::label(unit)))
            .absolute()
            .inset_0()
            .border_1()
            .border_dashed()
            .border_color(p.text_muted)
            .rounded(px(metrics::RADIUS))
            .occlude()
            .cursor_grab()
            .on_drag(DraggedUnit { unit, from: Some(spot) }, |dragged, _, _, cx| {
                dragged.preview(cx)
            })
            .on_drag_move(cx.listener(move |this, event: &DragMoveEvent<DraggedUnit>, _, _| {
                this.track_piece(spot, event.bounds);
            }));
        div()
            .relative()
            .min_w(px(MIN_PIECE))
            .h(px(MIN_PIECE + 2. * OUTLINE_PAD))
            .p(px(OUTLINE_PAD))
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(10.))
            .when(grows, Styled::flex_1)
            .when(!grows, Styled::flex_none)
            .children(inner)
            .child(handle)
            .into_any_element()
    }

    /// Makes `area`, the bar holding `zone`, follow drags over it and take their drops.
    pub(in crate::main_window) fn edit_drop_zone<E: InteractiveElement + 'static>(
        area: E,
        zone: Zone,
        cx: &mut Context<Self>,
    ) -> E {
        area.on_drag_move(cx.listener(move |this, event: &DragMoveEvent<DraggedUnit>, _, cx| {
            this.track_zone(zone, event.event.position, event.bounds, cx);
        }))
        .on_drop(cx.listener(|this, dragged: &DraggedUnit, _, cx| this.drop_unit(dragged, cx)))
    }

    /// The place for a second control-bar row above the bar, while a unit is dragged and there
    /// is room for one.
    pub(in crate::main_window) fn render_new_row(
        &self,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let customizing = self.customizing.as_ref()?;
        if !cx.has_active_drag() || self.shown_layout().rows.len() >= MAX_ROWS {
            return None;
        }
        let lit = customizing.target == Some(Target::NewRow);
        let row = div()
            .h(px(CONTROL_BAR_HEIGHT))
            .mx(px(20.))
            .my(px(6.))
            .flex()
            .items_center()
            .justify_center()
            .border_1()
            .border_dashed()
            .border_color(if lit { p.accent } else { p.text_muted })
            .rounded(px(metrics::RADIUS))
            .text_size(px(13.))
            .text_color(p.text_muted)
            .child("Drop here for a second row")
            .on_drag_move(cx.listener(|this, event: &DragMoveEvent<DraggedUnit>, _, cx| {
                let over = event.bounds.contains(&event.event.position);
                this.track_drop_area(Some(Target::NewRow), over, cx);
            }))
            .on_drop(cx.listener(|this, dragged: &DraggedUnit, _, cx| this.drop_unit(dragged, cx)));
        Some(row)
    }
}

/// A unique id for the outline of the piece at `spot`.
fn piece_id(spot: Spot) -> ElementId {
    let zone = match spot.zone {
        Zone::TitleBar => "customize-title-bar",
        Zone::Row(0) => "customize-row-0",
        Zone::Row(_) => "customize-row-1",
    };
    (zone, spot.index).into()
}

fn drop_marker(p: &Palette) -> AnyElement {
    div().w(px(MARKER_WIDTH)).h(px(MARKER_HEIGHT)).flex_none().bg(p.accent).into_any_element()
}

/// `name` in muted text: an item in edit mode that shows nothing yet, or the title in line.
pub(in crate::main_window) fn placeholder(name: SharedString, p: &Palette) -> AnyElement {
    div()
        .flex_none()
        .px(px(4.))
        .whitespace_nowrap()
        .text_size(px(13.))
        .text_color(p.text_muted)
        .child(name)
        .into_any_element()
}
