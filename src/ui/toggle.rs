//! Toggle and checkbox (S0.4.4).
//!
//! The toggle is the designed square-knob pill (`shared/components/toggle-*`,
//! 68×32): a well holding two 30×26 halves. OFF shows a gray knob on the left and
//! "OFF" on the right; ON shows "ON" in the accent on the accent tint and an accent
//! knob on the right.

use std::rc::Rc;

use gpui::{App, Div, ElementId, SharedString, Window, accesskit, div, prelude::*, px};

use super::icon::icon;
use super::{focus, metrics};
use crate::assets::icons;
use crate::theme::{self, Palette};

/// Called with the control's new state.
type ChangeHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;

/// The toggle's width, from the design.
pub const TOGGLE_WIDTH: f32 = 68.;
/// The toggle's height, from the design.
pub const TOGGLE_HEIGHT: f32 = 32.;

/// An ON/OFF switch. The owner keeps the state and changes it from
/// [`Self::on_change`].
#[derive(IntoElement)]
pub struct Toggle {
    id: ElementId,
    on: bool,
    label: SharedString,
    disabled: bool,
    on_change: Option<ChangeHandler>,
}

impl Toggle {
    /// `label` names the setting for screen readers; it isn't drawn.
    pub fn new(id: impl Into<ElementId>, on: bool, label: impl Into<SharedString>) -> Self {
        Self { id: id.into(), on, label: label.into(), disabled: false, on_change: None }
    }

    /// Dims the toggle and ignores clicks and focus.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Called with the new state.
    pub fn on_change(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

/// One of the toggle's two halves: the knob or the ON/OFF label.
fn half() -> Div {
    div()
        .flex_1()
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(3.))
        .text_size(px(12.))
}

impl RenderOnce for Toggle {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let on = self.on;
        let enabled = !self.disabled;

        let knob = half().bg(if on { p.accent_strong } else { p.knob });
        let text = half()
            .bg(if on { p.accent_tint } else { p.control })
            .text_color(if on { p.accent_on_tint } else { p.text_muted })
            .child(if on { "ON" } else { "OFF" });

        let toggle = div()
            .id(self.id)
            .w(px(TOGGLE_WIDTH))
            .h(px(TOGGLE_HEIGHT))
            .flex_none()
            .flex()
            .flex_row()
            .gap(px(2.))
            .p(px(1.))
            .rounded(px(metrics::RADIUS + 2.))
            .bg(p.well)
            .border_1()
            .border_color(p.well)
            .role(accesskit::Role::Switch)
            .aria_label(self.label)
            .aria_toggled(if on { accesskit::Toggled::True } else { accesskit::Toggled::False })
            .map(|t| if on { t.child(text).child(knob) } else { t.child(knob).child(text) })
            .when(!enabled, |t| t.opacity(0.4));

        if !enabled {
            return toggle;
        }
        let toggle =
            focus::ring(toggle.tab_index(0).key_context(focus::CONTROL).cursor_pointer(), &p);
        match self.on_change {
            Some(f) => toggle.on_click(move |_, window, cx| f(&!on, window, cx)),
            None => toggle,
        }
    }
}

/// A checkbox with a label; the whole row is the click target.
#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    checked: bool,
    label: SharedString,
    disabled: bool,
    on_change: Option<ChangeHandler>,
}

impl Checkbox {
    /// An enabled checkbox showing `label` next to the box.
    pub fn new(id: impl Into<ElementId>, checked: bool, label: impl Into<SharedString>) -> Self {
        Self { id: id.into(), checked, label: label.into(), disabled: false, on_change: None }
    }

    /// Dims the checkbox and ignores clicks and focus.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Called with the new checked state.
    pub fn on_change(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

/// The square box, with a check mark when `checked`.
fn check_box(checked: bool, p: &Palette) -> Div {
    div()
        .size(px(20.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .bg(if checked { p.accent_tint } else { p.well })
        .border_1()
        .border_color(if checked { p.accent_fg } else { p.key_border })
        .when(checked, |b| b.child(icon(icons::CHECK, 16., p.accent_on_tint)))
}

impl RenderOnce for Checkbox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let checked = self.checked;
        let enabled = !self.disabled;

        let row = div()
            .id(self.id)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.))
            .min_w_0()
            .px(px(2.))
            .rounded(px(metrics::RADIUS))
            .border_2()
            .border_color(gpui::transparent_black())
            .role(accesskit::Role::CheckBox)
            .aria_label(self.label.clone())
            .aria_toggled(if checked {
                accesskit::Toggled::True
            } else {
                accesskit::Toggled::False
            })
            .child(check_box(checked, &p))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(px(metrics::TEXT))
                    .text_color(p.text)
                    .child(self.label),
            )
            .when(!enabled, |r| r.opacity(0.4));

        if !enabled {
            return row;
        }
        let row = focus::ring(row.tab_index(0).key_context(focus::CONTROL).cursor_pointer(), &p);
        match self.on_change {
            Some(f) => row.on_click(move |_, window, cx| f(&!checked, window, cx)),
            None => row,
        }
    }
}
