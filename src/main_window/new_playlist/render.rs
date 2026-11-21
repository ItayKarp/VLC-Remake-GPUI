//! How the New Playlist dialog looks: a dark header with the title and ✕, the two sections, and
//! Cancel at the left with Save at the right. Sizes are the design's, at half its pixels.

use gpui::{
    ClickEvent, Context, Div, ExternalPaths, FontWeight, ObjectFit, SharedString, Stateful, Window,
    accesskit, div, img, prelude::*, px, svg,
};

use super::{NewPlaylistDialog, Thumbnail};
use crate::assets::icons;
use crate::theme::{self, Palette};
use crate::ui::button::{Button, ButtonStyle, IconButton};
use crate::ui::{focus, metrics};

/// The dialog's width when the window has room.
const WIDTH: f32 = 712.;
/// Space kept between the dialog and the window edges.
pub(super) const WINDOW_MARGIN: f32 = 24.;
/// The header's height, and the space inside the dialog's edges.
const HEADER_HEIGHT: f32 = 50.;
const PADDING: f32 = 20.;
/// The thumbnail well's side, and the "+" buttons' sides.
const THUMBNAIL: f32 = 110.;
/// How far the thumbnail well sits below the section heading's top.
const THUMBNAIL_TOP: f32 = 9.;
const PLUS_BUTTON: f32 = 30.;
const PLUS_ICON: f32 = 20.;
/// The drop zone's height, and the width of the "+" drawn in it.
const DROP_ZONE_HEIGHT: f32 = 184.;
const DROP_ZONE_PLUS_WIDTH: f32 = 84.;
/// The space above Cancel and Save, and their width.
const FOOTER_GAP: f32 = 42.;
const FOOTER_BUTTON_WIDTH: f32 = 100.;
/// The section headings' and descriptions' text sizes.
const HEADING_TEXT: f32 = 17.;
const DESCRIPTION_TEXT: f32 = 14.;

impl Render for NewPlaylistDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let viewport = window.viewport_size();
        let body = div()
            .id("new-playlist-body")
            .flex_initial()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(PADDING))
            .pt(px(PADDING))
            .flex()
            .flex_col()
            .gap(px(41.))
            .child(self.details(&p, cx))
            .child(self.select_files(&p, cx));
        div()
            .id("new-playlist")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .role(accesskit::Role::Dialog)
            .aria_label("New Playlist")
            .w(px(WIDTH))
            .max_w(viewport.width - px(2. * self.window_margin))
            .max_h(viewport.height - px(2. * self.window_margin))
            .flex()
            .flex_col()
            .rounded(px(metrics::RADIUS_LARGE + 4.))
            .overflow_hidden()
            .bg(p.popover)
            .border_1()
            .border_color(p.divider)
            .shadow_lg()
            .text_color(p.text)
            .child(Self::header(&p, cx))
            .child(body)
            .child(self.footer(cx))
    }
}

impl NewPlaylistDialog {
    /// "New Playlist" and ✕ on a darker band.
    fn header(p: &Palette, cx: &mut Context<Self>) -> Div {
        let close = IconButton::new("new-playlist-close", icons::CLOSE, "Close")
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.cancel(window, cx)));
        div()
            .flex_none()
            .h(px(HEADER_HEIGHT))
            .flex()
            .items_center()
            .justify_between()
            .pl(px(PADDING))
            .pr(px(PADDING - 6.))
            .bg(p.sunken)
            .text_size(px(DESCRIPTION_TEXT + 1.))
            .text_color(p.text)
            .child("New Playlist")
            .child(close)
    }

    /// Playlist Details: the title field and the thumbnail well.
    fn details(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let text = section(
            "Playlist Details",
            "The playlist will be saved and displayed with this title and thumbnail.",
            p,
        )
        .flex_1()
        .min_w_0()
        .child(div().pt(px(24.)).child(self.title.clone()));
        let refused = match &self.thumbnail {
            Thumbnail::Refused(why) => Some(div().text_color(p.danger).child(*why)),
            _ => None,
        };
        let thumbnail = div()
            .flex_none()
            .mt(px(THUMBNAIL_TOP))
            .flex()
            .flex_col()
            .items_end()
            .gap(px(4.))
            .text_size(px(metrics::TEXT_SMALL))
            .child(self.thumbnail_well(p, cx))
            .children(refused);
        div().flex().items_start().gap(px(20.)).child(text).child(thumbnail)
    }

    /// The thumbnail well: the picked image, or a "+" that picks one.
    fn thumbnail_well(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let picked = match &self.thumbnail {
            Thumbnail::Picked(path) => Some(
                img(path.clone()).absolute().inset_0().size_full().object_fit(ObjectFit::Cover),
            ),
            _ => None,
        };
        let label = if picked.is_some() { "Change thumbnail" } else { "Choose thumbnail" };
        let plus = IconButton::new("new-playlist-thumbnail", icons::ADD, label)
            .size(PLUS_BUTTON, PLUS_ICON)
            .disabled(self.thumbnail == Thumbnail::Checking)
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.pick_thumbnail(window, cx);
            }));
        div()
            .relative()
            .size(px(THUMBNAIL))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(metrics::RADIUS_LARGE))
            .overflow_hidden()
            .bg(p.well)
            .border_1()
            .border_color(p.divider)
            .children(picked)
            .child(div().rounded(px(metrics::RADIUS)).bg(p.control).child(plus))
    }

    /// Select Files: the drop zone and what the last addition left out.
    fn select_files(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let note = self.note.clone().map(|note| {
            div().pt(px(8.)).text_size(px(metrics::TEXT_SMALL)).text_color(p.text_muted).child(note)
        });
        section("Select Files", "These files will be added to the playlist.", p)
            .child(div().pt(px(20.)).child(self.drop_zone(p, cx)))
            .children(note)
    }

    /// The dashed zone: drop files on it, or click it (or press Enter on it) to pick them.
    fn drop_zone(&self, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let accent = p.accent_fg;
        let plus = div()
            .w(px(DROP_ZONE_PLUS_WIDTH))
            .h(px(PLUS_BUTTON))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(metrics::RADIUS))
            .bg(p.control)
            .child(svg().path(icons::ADD).size(px(PLUS_ICON)).text_color(p.icon));
        let zone = div()
            .id("new-playlist-drop-zone")
            .tab_index(0)
            .key_context(focus::CONTROL)
            .role(accesskit::Role::Button)
            .aria_label("Add files")
            .h(px(DROP_ZONE_HEIGHT))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(16.))
            .px(px(PADDING))
            .rounded(px(metrics::RADIUS))
            .bg(p.well)
            .border_2()
            .border_dashed()
            .border_color(p.border)
            .cursor_pointer()
            .text_size(px(DESCRIPTION_TEXT + 1.))
            .text_color(p.text_muted)
            .drag_over::<ExternalPaths>(move |style, _, _, _| style.border_color(accent))
            .on_drop(cx.listener(Self::drop_files))
            .on_click(cx.listener(|_, _: &ClickEvent, window, cx| Self::pick_files(window, cx)))
            .child(div().text_center().child(self.drop_zone_text()))
            .child(plus);
        focus::ring(zone, p)
    }

    /// What the drop zone says: how to add files, how many are in, or that some are coming.
    fn drop_zone_text(&self) -> SharedString {
        if !self.adding.is_idle() {
            return "Adding files…".into();
        }
        match self.files.len() {
            0 => "Drag files into the box or click here to select files.".into(),
            1 => "1 file will be added. Drag in more or click here to select more.".into(),
            n => format!("{n} files will be added. Drag in more or click here to select more.")
                .into(),
        }
    }

    /// Cancel at the left, Save at the right.
    fn footer(&self, cx: &mut Context<Self>) -> Div {
        let can_save = self.can_save(cx);
        let cancel = Button::new("new-playlist-cancel", "Cancel")
            .style(ButtonStyle::Filled)
            .width(FOOTER_BUTTON_WIDTH)
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.cancel(window, cx)));
        let save_style = if can_save { ButtonStyle::Primary } else { ButtonStyle::Ghost };
        let save = Button::new("new-playlist-save", "Save")
            .style(save_style)
            .width(FOOTER_BUTTON_WIDTH)
            .disabled(!can_save)
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.save(window, cx)));
        div()
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .px(px(PADDING))
            .pt(px(FOOTER_GAP))
            .pb(px(PADDING))
            .child(cancel)
            .child(save)
    }
}

/// A section's heading and description, as a column to add its controls to.
fn section(heading: &'static str, description: &'static str, p: &Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .text_size(px(HEADING_TEXT))
                .font_weight(FontWeight::NORMAL)
                .text_color(p.text_strong)
                .child(heading),
        )
        .child(
            div()
                .pt(px(8.))
                .text_size(px(DESCRIPTION_TEXT))
                .text_color(p.text_muted)
                .child(description),
        )
}
