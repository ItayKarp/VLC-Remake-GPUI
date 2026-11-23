//! How the Add Subtitles dialog looks: a dark band with the title, ✕ and the header row, the
//! list, and Cancel at the left with Save at the right. Sizes are the design's, at half its
//! pixels.

use gpui::{
    AnyElement, ClickEvent, Context, Div, IntoElement, SharedString, Window, accesskit, div,
    prelude::*, px,
};

use super::{AddSubtitlesDialog, Body};
use crate::assets::icons;
use crate::theme::{self, Palette};
use crate::ui::button::IconButton;
use crate::ui::icon::icon;
use crate::ui::{metrics, popover};

/// The dialog's size when the window has room.
const WIDTH: f32 = 1048.;
const HEIGHT: f32 = 800.;
/// Space kept between the dialog and the window edges.
pub(super) const WINDOW_MARGIN: f32 = 24.;
/// The title line's height, and the space around the header row.
const TITLE_HEIGHT: f32 = 50.;
pub(super) const HEADER_INSET: f32 = 9.;
/// The space inside the dialog's edges around the list and the footer.
pub(super) const PADDING: f32 = 20.;
/// A list row's height, and the space between rows.
pub(super) const ROW_HEIGHT: f32 = 36.;
pub(super) const ROW_GAP: f32 = 4.;
/// A list row's icon, and the space between it and the name.
pub(super) const ROW_ICON: f32 = 22.;
pub(super) const ROW_ICON_GAP: f32 = 24.;

impl Render for AddSubtitlesDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let viewport = window.viewport_size();
        let menu = self.menu.as_ref().map(|(position, menu, _)| {
            popover::at_position(*position, 1, menu.clone()).into_any_element()
        });
        let body = div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .pt(px(PADDING + 3.))
            .child(self.body(&p, cx));
        div()
            .id("add-subtitles")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .role(accesskit::Role::Dialog)
            .aria_label("Add Subtitles")
            .w(px(WIDTH))
            .h(px(HEIGHT))
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
            .child(self.band(&p, cx))
            .child(body)
            .child(self.footer(&p, cx))
            .children(menu)
    }
}

impl AddSubtitlesDialog {
    /// The dark band: the title and ✕, then the Target, Origin and context fields.
    fn band(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let close = IconButton::new("add-subtitles-close", icons::CLOSE, "Close")
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.cancel(cx)));
        let title = div()
            .h(px(TITLE_HEIGHT))
            .flex()
            .items_center()
            .justify_between()
            .pl(px(PADDING + 2.))
            .pr(px(PADDING - 6.))
            .text_size(px(metrics::TEXT - 1.))
            .text_color(p.text_muted)
            .child("Add Subtitles")
            .child(close);
        div()
            .flex_none()
            .flex()
            .flex_col()
            .pb(px(HEADER_INSET))
            .bg(p.sunken)
            .child(title)
            .child(self.header_row(p, cx))
    }

    /// The list for the target and origin.
    fn body(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        match &self.body {
            Body::Loading => note("Reading the folder…", p).into_any_element(),
            Body::Files(files) => Self::render_files(files, p, cx),
            Body::Pairs(pairs) => Self::render_pairs(pairs, p, cx),
            Body::Search(search) => Self::render_search(search, p, cx),
            Body::Link => self.render_link(p, cx),
        }
    }
}

/// A line of muted text where the list would be.
pub(super) fn note(text: impl Into<SharedString>, p: &Palette) -> Div {
    div()
        .px(px(PADDING + 6.))
        .py(px(8.))
        .text_size(px(metrics::TEXT))
        .text_color(p.text_muted)
        .child(text.into())
}

/// A list row's icon and name, the name cut short with an ellipsis when it doesn't fit.
pub(super) fn icon_and_name(path: &'static str, name: SharedString, p: &Palette) -> Div {
    div()
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(ROW_ICON_GAP))
        .child(div().flex_none().child(icon(path, ROW_ICON, p.icon)))
        .child(div().min_w_0().truncate().text_size(px(metrics::TEXT + 1.)).child(name))
}

/// A path's file name, for a row.
pub(super) fn file_name(path: &std::path::Path) -> SharedString {
    path.file_name()
        .map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into_owned())
        .into()
}
