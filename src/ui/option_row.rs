//! Option rows (S0.4.4a): the rows inside dropdowns and menus.
//!
//! `shared/components/option-*`: default is transparent, hover and selected are
//! `#1F1F1F`, and the selected row ends with an accent ✓ (`dropdown-open`).
//! Menus add a leading icon (`library/menus/*`), a shortcut hint and a submenu
//! chevron.

use gpui::{App, ElementId, Hsla, SharedString, Window, accesskit, div, prelude::*, px};

use super::icon::icon;
use super::metrics;
use crate::assets::icons;
use crate::theme;

/// Labels longer than this many characters get a tooltip with the full text.
const LONG_LABEL: usize = 32;

#[derive(IntoElement)]
pub struct OptionRow {
    id: ElementId,
    label: SharedString,
    icon: Option<&'static str>,
    /// Shows the ✓ (dropdown value, checked menu item, chosen radio item).
    checked: bool,
    /// Keyboard highlight; the mouse highlight is the hover style.
    highlighted: bool,
    disabled: bool,
    shortcut: Option<SharedString>,
    submenu: bool,
    height: f32,
    /// Reserve the ✓ column even when unchecked, so labels line up in groups.
    check_column: bool,
    /// Reserve the icon column when there's no icon, so labels line up with
    /// rows that have one.
    reserve_icon: bool,
}

impl OptionRow {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            checked: false,
            highlighted: false,
            disabled: false,
            shortcut: None,
            submenu: false,
            height: metrics::FIELD_HEIGHT,
            check_column: true,
            reserve_icon: false,
        }
    }

    pub fn icon(mut self, icon: Option<&'static str>) -> Self {
        self.icon = icon;
        self
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn highlighted(mut self, highlighted: bool) -> Self {
        self.highlighted = highlighted;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn shortcut(mut self, shortcut: Option<SharedString>) -> Self {
        self.shortcut = shortcut;
        self
    }

    pub fn submenu(mut self, submenu: bool) -> Self {
        self.submenu = submenu;
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    pub fn check_column(mut self, reserve: bool) -> Self {
        self.check_column = reserve;
        self
    }

    pub fn reserve_icon(mut self, reserve: bool) -> Self {
        self.reserve_icon = reserve;
        self
    }
}

impl RenderOnce for OptionRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let hover = p.hover;
        let fg: Hsla = if self.highlighted || self.checked { p.text_strong } else { p.text };
        // Long labels truncate and show the full text on hover. GPUI doesn't say
        // whether text was truncated, so go by length: about what fits a menu.
        let long_label = (self.label.chars().count() > LONG_LABEL).then(|| self.label.clone());

        div()
            .id(self.id)
            .h(px(self.height))
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .px(px(14.))
            .rounded(px(metrics::RADIUS + 2.))
            .text_size(px(metrics::TEXT))
            .text_color(fg)
            .role(accesskit::Role::MenuItem)
            .aria_label(self.label.clone())
            .aria_selected(self.checked)
            .when(self.highlighted, |r| r.bg(hover))
            .when(!self.disabled, |r| r.cursor_pointer().hover(move |s| s.bg(hover)))
            .when(self.disabled, |r| r.opacity(0.4))
            .when_some(self.icon, |r, path| r.child(icon(path, metrics::ICON, p.accent_fg)))
            .when(self.icon.is_none() && self.reserve_icon, |r| {
                r.child(div().size(px(metrics::ICON)).flex_none())
            })
            .child(
                div()
                    .id("label")
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .when_some(long_label, |l, label| {
                        l.tooltip(super::tooltip::text(label, None::<SharedString>))
                    })
                    .child(self.label),
            )
            .when_some(self.shortcut, |r, hint| {
                r.child(
                    div()
                        .flex_none()
                        .text_size(px(metrics::TEXT_SMALL))
                        .text_color(p.text_muted)
                        .child(hint),
                )
            })
            .when(self.submenu, |r| r.child(icon(icons::CHEVRON_RIGHT, 18., p.icon)))
            .when(self.checked, |r| r.child(icon(icons::CHECK, 22., p.accent_fg)))
            .when(!self.checked && self.check_column && !self.submenu, |r| {
                r.child(div().size(px(22.)).flex_none())
            })
    }
}
