//! Segmented "values" controls and the stacked choice (S0.4.4a).
//!
//! `shared/components/values-option_*`: a well holding 2 or 3 equal segments
//! (Relative | Pixels, Dark | Light | System). The selected one is the accent tint
//! with accent text. `values-variant2` is the stacked choice: full-width rows, one
//! of which can hold an inline field ("Always starts at: [200%]").
//!
//! Both are radio groups: one tab stop, arrow keys move the selection.

use std::rc::Rc;

use gpui::{AnyElement, App, ElementId, SharedString, Window, accesskit, div, prelude::*, px};

use super::focus::{self, Axis, Layout};
use super::metrics;
use crate::theme;

type SelectHandler = Rc<dyn Fn(&usize, &mut Window, &mut App)>;

/// Applies an arrow key to a radio group and reports the new selection.
fn radio_keys(
    group: gpui::Stateful<gpui::Div>,
    selected: usize,
    len: usize,
    axis: Axis,
    on_select: Option<SelectHandler>,
) -> gpui::Stateful<gpui::Div> {
    let Some(on_select) = on_select else {
        return group;
    };
    group.on_key_down(move |event, window, cx| {
        let Some(movement) = focus::group_key(event, axis) else {
            return;
        };
        if let Some(next) = focus::step(Some(selected), len, movement, Layout::LIST, |_| true) {
            cx.stop_propagation();
            if next != selected {
                on_select(&next, window, cx);
            }
        }
    })
}

#[derive(IntoElement)]
pub struct Segmented {
    id: ElementId,
    options: Vec<SharedString>,
    selected: usize,
    label: SharedString,
    on_select: Option<SelectHandler>,
}

impl Segmented {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        options: impl IntoIterator<Item = impl Into<SharedString>>,
        selected: usize,
    ) -> Self {
        Self {
            id: id.into(),
            options: options.into_iter().map(Into::into).collect(),
            selected,
            label: label.into(),
            on_select: None,
        }
    }

    pub fn on_select(mut self, f: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Segmented {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let selected = self.selected;
        let len = self.options.len();
        let on_select = self.on_select.clone();

        let segments = self.options.into_iter().enumerate().map(|(i, label)| {
            let is_selected = i == selected;
            let on_select = on_select.clone();
            let hover = p.button_hover;
            div()
                .id(i)
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .px(px(8.))
                .rounded(px(metrics::RADIUS))
                .text_size(px(metrics::TEXT))
                .bg(if is_selected { p.accent_tint } else { p.control })
                .text_color(if is_selected { p.accent_on_tint } else { p.text_muted })
                .role(accesskit::Role::RadioButton)
                .aria_selected(is_selected)
                .when(!is_selected, |s| s.cursor_pointer().hover(move |s| s.bg(hover)))
                .when_some(on_select, |s, f| {
                    s.on_click(move |_, window, cx| {
                        if !is_selected {
                            f(&i, window, cx);
                        }
                    })
                })
                .child(div().truncate().child(label))
        });

        let group = div()
            .id(self.id)
            .h(px(metrics::FIELD_HEIGHT))
            .flex()
            .flex_row()
            .gap(px(metrics::WELL_INSET))
            .p(px(metrics::WELL_INSET))
            .rounded(px(metrics::RADIUS + 2.))
            .bg(p.well)
            .border_2()
            .border_color(p.well)
            .tab_index(0)
            .role(accesskit::Role::RadioGroup)
            .aria_label(self.label)
            .children(segments);
        radio_keys(focus::ring(group, &p), selected, len, Axis::Horizontal, self.on_select)
    }
}

/// A full-width row in a [`StackedChoice`].
pub struct Choice {
    label: SharedString,
    field: Option<AnyElement>,
}

impl Choice {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self { label: label.into(), field: None }
    }

    /// An inline field shown at the right of the row (a `TextInput`, usually).
    pub fn field(mut self, field: impl IntoElement) -> Self {
        self.field = Some(field.into_any_element());
        self
    }
}

#[derive(IntoElement)]
pub struct StackedChoice {
    id: ElementId,
    label: SharedString,
    choices: Vec<Choice>,
    selected: usize,
    on_select: Option<SelectHandler>,
}

impl StackedChoice {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        choices: Vec<Choice>,
        selected: usize,
    ) -> Self {
        Self { id: id.into(), label: label.into(), choices, selected, on_select: None }
    }

    pub fn on_select(mut self, f: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for StackedChoice {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let selected = self.selected;
        let len = self.choices.len();
        let on_select = self.on_select.clone();

        let rows = self.choices.into_iter().enumerate().map(|(i, choice)| {
            let is_selected = i == selected;
            let on_select = on_select.clone();
            let hover = p.button_hover;
            div()
                .id(i)
                .h(px(36.))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(10.))
                .pl(px(12.))
                .pr(px(2.))
                .py(px(2.))
                .rounded(px(metrics::RADIUS))
                .text_size(px(metrics::TEXT))
                .bg(if is_selected { p.accent_tint } else { p.control })
                .text_color(if is_selected { p.accent_on_tint } else { p.text_muted })
                .role(accesskit::Role::RadioButton)
                .aria_selected(is_selected)
                .when(!is_selected, |s| s.cursor_pointer().hover(move |s| s.bg(hover)))
                .when_some(on_select, |s, f| {
                    s.on_click(move |_, window, cx| {
                        if !is_selected {
                            f(&i, window, cx);
                        }
                    })
                })
                .child(div().flex_none().child(choice.label))
                .when_some(choice.field, |s, field| {
                    s.child(div().flex_1().min_w_0().h_full().child(field))
                })
        });

        let group = div()
            .id(self.id)
            .flex()
            .flex_col()
            .gap(px(metrics::WELL_INSET))
            .p(px(metrics::WELL_INSET))
            .rounded(px(metrics::RADIUS + 2.))
            .bg(p.well)
            .border_2()
            .border_color(p.well)
            .tab_index(0)
            .role(accesskit::Role::RadioGroup)
            .aria_label(self.label)
            .children(rows);
        radio_keys(focus::ring(group, &p), selected, len, Axis::Vertical, self.on_select)
    }
}
