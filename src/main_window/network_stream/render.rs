//! How the Open Network Stream dialog looks: a dark header with the title and ✕, the two
//! sections, and Cancel at the left with Save at the right. Sizes are the design's, at half its
//! pixels, as the New Playlist dialog's are; it is wider, for long links.

use gpui::{
    ClickEvent, Context, Div, FontWeight, SharedString, Window, accesskit, div, prelude::*, px,
};

use super::NetworkStreamDialog;
use crate::assets::icons;
use crate::i18n::DirectionalStyle;
use crate::theme::text_px;
use crate::theme::{self, Palette};
use crate::ui::button::{Button, ButtonStyle, IconButton};
use crate::ui::metrics;

/// The dialog's width when the window has room.
const WIDTH: f32 = 1048.;
/// Space kept between the dialog and the window edges.
pub(super) const WINDOW_MARGIN: f32 = 24.;
/// The header's height, and the space inside the dialog's edges.
const HEADER_HEIGHT: f32 = 50.;
const PADDING: f32 = 20.;
/// The space between the sections, above each field, and above Cancel and Save.
const SECTION_GAP: f32 = 36.;
const FIELD_GAP: f32 = 24.;
const FOOTER_GAP: f32 = 80.;
/// Cancel's and Save's width.
const FOOTER_BUTTON_WIDTH: f32 = 100.;
/// The section headings' and descriptions' text sizes.
const HEADING_TEXT: f32 = 17.;
const DESCRIPTION_TEXT: f32 = 14.;

impl Render for NetworkStreamDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let viewport = window.viewport_size();
        let body = div()
            .id("network-stream-body")
            .flex_initial()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(PADDING))
            .pt(px(PADDING))
            .flex_row_start()
            .flex_col()
            .gap(px(SECTION_GAP))
            .child(self.links_section(&p, cx))
            .child(self.name_section(&p));
        div()
            .id("network-stream")
            .role(accesskit::Role::Dialog)
            .aria_label(tr!("Open Network Stream"))
            .w(px(WIDTH))
            .max_w(viewport.width - px(2. * self.window_margin))
            .max_h(viewport.height - px(2. * self.window_margin))
            .flex_row_start()
            .flex_col()
            .rounded(px(metrics::RADIUS_LARGE + 4.))
            .overflow_hidden()
            .bg(p.popover)
            .border_1()
            .border_color(p.divider)
            .shadow_lg()
            .text_color(p.text)
            .child(header(&p, cx))
            .child(body)
            .child(self.footer(cx))
    }
}

impl NetworkStreamDialog {
    /// Network URL: the links field and the notes on what it leaves out.
    fn links_section(&self, p: &Palette, cx: &Context<Self>) -> Div {
        let notes = self.notes(cx).into_iter().map(|note| {
            div().text_size(text_px(metrics::TEXT_SMALL)).text_color(p.text_muted).child(note)
        });
        section(
            heading(tr!("Network URL"), None, p),
            tr!("You can enter multiple links to load."),
            p,
        )
        .child(div().pt(px(FIELD_GAP)).child(self.urls.clone()))
        .child(div().pt(px(8.)).flex_row_start().flex_col().gap(px(4.)).children(notes))
    }

    /// Playlist Name (optional).
    fn name_section(&self, p: &Palette) -> Div {
        let description = tr!("The playlist will be saved and displayed with this title.");
        section(heading(tr!("Playlist Name"), Some(tr!("(optional)")), p), description, p)
            .child(div().pt(px(FIELD_GAP)).child(self.name.clone()))
    }

    /// Cancel at the left, Save at the right.
    fn footer(&self, cx: &mut Context<Self>) -> Div {
        let can_save = self.saved(cx).is_some();
        let cancel = Button::new("network-stream-cancel", tr!("Cancel"))
            .style(ButtonStyle::Filled)
            .width(FOOTER_BUTTON_WIDTH)
            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(gpui::DismissEvent)));
        let save_style = if can_save { ButtonStyle::Primary } else { ButtonStyle::Ghost };
        let save = Button::new("network-stream-save", tr!("Save"))
            .style(save_style)
            .width(FOOTER_BUTTON_WIDTH)
            .disabled(!can_save)
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.save(window, cx)));
        div()
            .flex_none()
            .flex_row_start()
            .items_center()
            .justify_between()
            .px(px(PADDING))
            .pt(px(FOOTER_GAP))
            .pb(px(PADDING))
            .child(cancel)
            .child(save)
    }
}

/// "Open Network Stream" and ✕ on a darker band.
fn header(p: &Palette, cx: &mut Context<NetworkStreamDialog>) -> Div {
    let close = IconButton::new("network-stream-close", icons::CLOSE, tr!("Close"))
        .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(gpui::DismissEvent)));
    div()
        .flex_none()
        .h(px(HEADER_HEIGHT))
        .flex_row_start()
        .items_center()
        .justify_between()
        .ps(px(PADDING))
        .pe(px(PADDING - 6.))
        .bg(p.sunken)
        .text_size(text_px(DESCRIPTION_TEXT + 1.))
        .text_color(p.text)
        .child(tr!("Open Network Stream"))
        .child(close)
}

/// A section's heading, with a muted `aside` after it.
fn heading(text: &'static str, aside: Option<&'static str>, p: &Palette) -> Div {
    div()
        .flex_row_start()
        .gap(px(5.))
        .text_size(text_px(HEADING_TEXT))
        .font_weight(FontWeight::NORMAL)
        .text_color(p.text_strong)
        .child(text)
        .children(aside.map(|aside| div().text_color(p.text_muted).child(aside)))
}

/// A section's heading and description, as a column to add its controls to.
fn section(heading: Div, description: &'static str, p: &Palette) -> Div {
    div().flex_row_start().flex_col().child(heading).child(
        div()
            .pt(px(8.))
            .text_size(text_px(DESCRIPTION_TEXT))
            .text_color(p.text_muted)
            .child(SharedString::from(description)),
    )
}
