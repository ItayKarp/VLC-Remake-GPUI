//! How a page lays out (`docs/ui/settings/screens/*`): section headings, then label/control
//! rows. A label has an `(i)` icon ([`InfoTip`]) whose tooltip is the setting's info text; the control sits in
//! a fixed column on the right. A disabled row is dimmed. The Advanced Options bar closes a page.

use std::ops::Range;

use gpui::{
    AnyElement, Div, FontWeight, HighlightStyle, SharedString, Stateful, StyledText, div,
    prelude::*, px,
};

use crate::assets::icons;
use crate::i18n::DirectionalStyle;
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::icon::icon;
use crate::ui::info_tip::InfoTip;
use crate::ui::{focus, metrics};

/// The page's side padding and the space under its last row.
pub const PAGE_PADDING: f32 = 30.;
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
pub(super) const DISABLED_OPACITY: f32 = 0.4;
/// The Advanced Options bar's height and the space above it.
const ADVANCED_HEIGHT: f32 = 36.;
const ADVANCED_TOP: f32 = 24.;

/// What a row shows besides its control.
pub(super) struct RowLabel {
    /// Identifies the row's elements.
    pub key: &'static str,
    /// The label.
    pub label: &'static str,
    /// The parts of the label a search matched, marked in the accent.
    pub highlights: Vec<Range<usize>>,
    /// The `(i)` tooltip; empty for none.
    pub info: &'static str,
    /// A line under the label ("Applies to the next file you open"), muted unless it warns of
    /// danger.
    pub note: Option<SharedString>,
    /// Whether the note is drawn in the danger color.
    pub note_is_danger: bool,
    /// Whether the row can be changed.
    pub enabled: bool,
}

/// A section's heading.
pub(super) fn section_title(title: &'static str, p: &Palette) -> Div {
    div()
        .pt(px(SECTION_TOP))
        .pb(px(SECTION_BOTTOM))
        .text_size(text_px(SECTION_TEXT))
        .text_color(p.text_strong)
        .child(crate::i18n::text(title))
}

/// A label/control row, dimmed while disabled.
pub(super) fn row(label: &RowLabel, control: AnyElement, p: &Palette) -> Div {
    let text = div()
        .flex_row_start()
        .items_center()
        .gap(px(INFO_GAP))
        .child(label_text(label, p))
        .when(!label.info.is_empty(), |text| text.child(info_icon(label)));
    let note_color = if label.note_is_danger { p.danger } else { p.text_muted };
    let note = label.note.clone().map(|note| {
        div().text_size(text_px(metrics::TEXT_SMALL)).text_color(note_color).child(note)
    });
    div()
        .min_h(px(ROW_HEIGHT))
        .py(px(4.))
        .flex_row_start()
        .items_center()
        .gap(px(ROW_GAP))
        .text_size(text_px(metrics::TEXT))
        .text_color(p.text)
        .when(!label.enabled, |row| row.opacity(DISABLED_OPACITY))
        .child(div().flex_1().min_w_0().flex_row_start().flex_col().child(text).children(note))
        .child(div().w(px(CONTROL_WIDTH)).flex_none().flex_row_start().justify_end().child(control))
}

/// The label, with the search's matches marked.
fn label_text(label: &RowLabel, p: &Palette) -> StyledText {
    let marked = HighlightStyle {
        color: Some(p.accent_on_tint),
        background_color: Some(p.accent_tint),
        font_weight: Some(FontWeight::SEMIBOLD),
        ..HighlightStyle::default()
    };
    let highlights = label.highlights.iter().map(|range| (range.clone(), marked));
    StyledText::new(label.label).with_highlights(highlights)
}

/// The `(i)` icon, showing the info text on hover and keyboard focus.
fn info_icon(label: &RowLabel) -> InfoTip {
    InfoTip::new(
        SharedString::from(format!("info-{}", label.key)),
        label.label,
        label.info,
        INFO_ICON,
    )
}

/// The bar that opens and closes the page's Advanced Options; Enter or Space toggles it.
pub(super) fn advanced_bar(open: bool, p: &Palette) -> Stateful<Div> {
    let hover = p.button_hover;
    let bar = div()
        .id("advanced-options")
        .mt(px(ADVANCED_TOP))
        .h(px(ADVANCED_HEIGHT))
        .flex_row_start()
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
        .aria_label(tr!("Advanced Options"))
        .text_size(text_px(metrics::TEXT))
        .text_color(p.text)
        .child(tr!("Advanced Options"))
        .child(icon(
            if open {
                icons::CHEVRON_DOWN
            } else {
                crate::i18n::mirrored(icons::CHEVRON_RIGHT, icons::CHEVRON_LEFT)
            },
            20.,
            p.icon,
        ));
    focus::ring(bar, p)
}

/// A muted line for a page without rows to show.
pub(super) fn empty_note(text: &'static str, p: &Palette) -> Div {
    div().pt(px(SECTION_TOP)).text_size(text_px(metrics::TEXT)).text_color(p.text_muted).child(text)
}
