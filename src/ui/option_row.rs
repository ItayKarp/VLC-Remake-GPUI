//! Option rows (S0.4.4a): the rows inside dropdowns and menus.
//!
//! `shared/components/option-*`: default is transparent, hover and selected are
//! `#1F1F1F`, and the selected row ends with an accent ✓ (`dropdown-open`).
//! Menus add a leading icon (`library/menus/*`), a shortcut hint and a submenu
//! chevron.

use gpui::{
    App, Div, ElementId, Hsla, SharedString, Stateful, Window, accesskit, div, prelude::*, px,
};

use super::icon::{icon, two_tone};
use super::metrics;
use crate::assets::{TwoTone, icons};
use crate::theme::{self, Palette};

/// Labels longer than this many characters get a tooltip with the full text.
const LONG_LABEL: usize = 32;
/// The size of the trailing ✓, and of the space reserved for it.
const CHECK_SIZE: f32 = 22.;

/// How the row responds to input.
#[derive(Clone, Copy, Default)]
struct Interaction {
    /// Keyboard highlight; the mouse highlight is the hover style.
    highlighted: bool,
    disabled: bool,
}

/// Columns kept empty when they have nothing to show, so labels line up in a group.
#[derive(Clone, Copy)]
struct Reserved {
    /// Reserve the ✓ column even when unchecked.
    check: bool,
    /// Reserve the icon column when there's no icon, to line up with rows that have
    /// one.
    icon: bool,
}

/// One row of a dropdown or menu: an optional icon, a label, and trailing marks.
#[derive(IntoElement)]
pub struct OptionRow {
    id: ElementId,
    label: SharedString,
    icon: Option<&'static str>,
    /// A two-tone glyph before the label instead of an icon (an equalizer preset's curve).
    glyph: Option<TwoTone>,
    /// Shows the ✓ (dropdown value, checked menu item, chosen radio item).
    checked: bool,
    interaction: Interaction,
    shortcut: Option<SharedString>,
    submenu: bool,
    height: f32,
    reserved: Reserved,
}

impl OptionRow {
    /// A plain, enabled row showing `label`, with the ✓ column reserved.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            glyph: None,
            checked: false,
            interaction: Interaction::default(),
            shortcut: None,
            submenu: false,
            height: metrics::FIELD_HEIGHT,
            reserved: Reserved { check: true, icon: false },
        }
    }

    /// Shows `icon` before the label.
    pub fn icon(mut self, icon: Option<&'static str>) -> Self {
        self.icon = icon;
        self
    }

    /// Shows the two-tone `glyph` before the label.
    pub fn glyph(mut self, glyph: Option<TwoTone>) -> Self {
        self.glyph = glyph;
        self
    }

    /// Shows the ✓ at the end of the row.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Shows the keyboard highlight.
    pub fn highlighted(mut self, highlighted: bool) -> Self {
        self.interaction.highlighted = highlighted;
        self
    }

    /// Dims the row and drops its hover style.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.interaction.disabled = disabled;
        self
    }

    /// Shows a keyboard shortcut hint after the label.
    pub fn shortcut(mut self, shortcut: Option<SharedString>) -> Self {
        self.shortcut = shortcut;
        self
    }

    /// Ends the row with a chevron: it opens a submenu.
    pub fn submenu(mut self, submenu: bool) -> Self {
        self.submenu = submenu;
        self
    }

    /// Sets the row's height (menus are more compact than dropdowns).
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// Whether to keep the ✓ column when the row isn't checked.
    pub fn check_column(mut self, reserve: bool) -> Self {
        self.reserved.check = reserve;
        self
    }

    /// Whether to keep the icon column when the row has no icon.
    pub fn reserve_icon(mut self, reserve: bool) -> Self {
        self.reserved.icon = reserve;
        self
    }
}

impl RenderOnce for OptionRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let hover = p.hover;
        let Interaction { highlighted, disabled } = self.interaction;
        let fg: Hsla = if highlighted || self.checked { p.text_strong } else { p.text };
        let check_spacer = !self.checked && self.reserved.check && !self.submenu;

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
            .when(highlighted, |r| r.bg(hover))
            .when(!disabled, |r| r.cursor_pointer().hover(move |s| s.bg(hover)))
            .when(disabled, |r| r.opacity(0.4))
            .when_some(self.icon, |r, path| r.child(icon(path, metrics::ICON, p.accent_fg)))
            .when_some(self.glyph, |r, glyph| {
                r.child(two_tone(glyph, metrics::ICON, p.icon, p.accent_fg))
            })
            .when(self.icon.is_none() && self.reserved.icon, |r| {
                r.child(div().size(px(metrics::ICON)).flex_none())
            })
            .child(label(self.label))
            .when_some(self.shortcut, |r, hint| r.child(shortcut_hint(hint, &p)))
            .when(self.submenu, |r| r.child(icon(icons::CHEVRON_RIGHT, 18., p.icon)))
            .when(self.checked, |r| r.child(icon(icons::CHECK, CHECK_SIZE, p.accent_fg)))
            .when(check_spacer, |r| r.child(div().size(px(CHECK_SIZE)).flex_none()))
    }
}

/// The label, truncated to fit. Long labels show the full text on hover: GPUI doesn't
/// say whether text was truncated, so go by length, about what fits a menu.
fn label(text: SharedString) -> Stateful<Div> {
    let long_label = (text.chars().count() > LONG_LABEL).then(|| text.clone());
    div()
        .id("label")
        .flex_1()
        .min_w_0()
        .truncate()
        .when_some(long_label, |l, label| {
            l.tooltip(super::tooltip::text(label, None::<SharedString>))
        })
        .child(text)
}

/// The muted keyboard shortcut after the label.
fn shortcut_hint(hint: SharedString, p: &Palette) -> Div {
    div().flex_none().text_size(px(metrics::TEXT_SMALL)).text_color(p.text_muted).child(hint)
}
