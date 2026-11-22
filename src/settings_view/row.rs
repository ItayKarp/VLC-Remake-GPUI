//! How a page lays out (`docs/ui/settings/screens/*`): section headings, then label/control
//! rows. A label has an `(i)` icon whose tooltip is the setting's info text; the control sits in
//! a fixed column on the right. A disabled row is dimmed. The Advanced Options bar closes a page.

use gpui::{AnyElement, Div, SharedString, Stateful, div, prelude::*, px};

use crate::assets::icons;
use crate::theme::Palette;
use crate::ui::icon::icon;
use crate::ui::{focus, metrics, tooltip};

/// The page's side padding and the space under its last row.
pub(super) const PAGE_PADDING: f32 = 30.;
/// The control column's width.
const CONTROL_WIDTH: f32 = 316.;
/// A row's height, and the space between the label and the control.
const ROW_HEIGHT: f32 = 42.;
const ROW_GAP: f32 = 24.;
/// The `(i)` icon's size and its space after the label.
const INFO_ICON: f32 = 16.;
const INFO_GAP: f32 = 10.;
/// Section headings: their text size and the space above and under them.
const SECTION_TEXT: f32 = 18.;
const SECTION_TOP: f32 = 30.;
const SECTION_BOTTOM: f32 = 8.;
/// How much a disabled row shows through.
const DISABLED_OPACITY: f32 = 0.4;
/// The Advanced Options bar's height and the space above it.
const ADVANCED_HEIGHT: f32 = 36.;
const ADVANCED_TOP: f32 = 24.;

/// What a row shows besides its control.
pub(super) struct RowLabel {
    /// Identifies the row's elements.
    pub key: &'static str,
    /// The label.
    pub label: &'static str,
    /// The `(i)` tooltip; empty for none.
    pub info: &'static str,
    /// A muted line under the label ("Applies to the next file you open").
    pub note: Option<&'static str>,
    /// Whether the row can be changed.
    pub enabled: bool,
}

/// A section's heading.
pub(super) fn section_title(title: &'static str, p: &Palette) -> Div {
    div()
        .pt(px(SECTION_TOP))
        .pb(px(SECTION_BOTTOM))
        .text_size(px(SECTION_TEXT))
        .text_color(p.text_strong)
        .child(title)
}

/// A label/control row, dimmed while disabled.
pub(super) fn row(label: &RowLabel, control: AnyElement, p: &Palette) -> Div {
    let text = div()
        .flex()
        .items_center()
        .gap(px(INFO_GAP))
        .child(label.label)
        .when(!label.info.is_empty(), |text| text.child(info_icon(label, p)));
    let note = label
        .note
        .map(|note| div().text_size(px(metrics::TEXT_SMALL)).text_color(p.text_muted).child(note));
    div()
        .min_h(px(ROW_HEIGHT))
        .py(px(4.))
        .flex()
        .items_center()
        .gap(px(ROW_GAP))
        .text_size(px(metrics::TEXT))
        .text_color(p.text)
        .when(!label.enabled, |row| row.opacity(DISABLED_OPACITY))
        .child(div().flex_1().min_w_0().flex().flex_col().child(text).children(note))
        .child(div().w(px(CONTROL_WIDTH)).flex_none().flex().justify_end().child(control))
}

/// The `(i)` icon, showing the info text on hover.
fn info_icon(label: &RowLabel, p: &Palette) -> Stateful<Div> {
    div()
        .id(SharedString::from(format!("info-{}", label.key)))
        .flex_none()
        .tooltip(tooltip::text(label.info, None::<SharedString>))
        .child(icon(icons::INFO, INFO_ICON, p.icon))
}

/// The bar that opens and closes the page's Advanced Options; Enter or Space toggles it.
pub(super) fn advanced_bar(open: bool, p: &Palette) -> Stateful<Div> {
    let hover = p.button_hover;
    let bar = div()
        .id("advanced-options")
        .mt(px(ADVANCED_TOP))
        .h(px(ADVANCED_HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(INFO_GAP))
        .rounded(px(metrics::RADIUS))
        .border_2()
        .border_color(gpui::transparent_black())
        .bg(p.panel)
        .hover(move |s| s.bg(hover))
        .cursor_pointer()
        .tab_index(0)
        .aria_expanded(open)
        .aria_label("Advanced Options")
        .text_size(px(metrics::TEXT))
        .text_color(p.text)
        .child("Advanced Options")
        .child(icon(if open { icons::CHEVRON_DOWN } else { icons::CHEVRON_RIGHT }, 20., p.icon));
    focus::ring(bar, p)
}

/// A muted line for a page without rows yet.
pub(super) fn empty_note(p: &Palette) -> Div {
    div()
        .pt(px(SECTION_TOP))
        .text_size(px(metrics::TEXT))
        .text_color(p.text_muted)
        .child("Nothing to set here yet.")
}
