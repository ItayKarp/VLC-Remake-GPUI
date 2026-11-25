//! The OpenSubtitles credit under the search (S9.9.4): OpenSubtitles' terms for integrators ask
//! applications on a free API key to show "Subtitles provided by OpenSubtitles.com", linking
//! to the site, where subtitles are searched or picked.

use gpui::{ClickEvent, Div, Stateful, accesskit, div, prelude::*, px};

use super::render::PADDING;
use crate::i18n::DirectionalStyle;
use crate::theme::{Palette, text_px};
use crate::ui::focus;
use crate::ui::metrics::{self, MIN_TARGET};

/// Where the credit links to.
const SITE: &str = "https://opensubtitles.com";

/// The credit line, a link to OpenSubtitles.com that Enter and Space press too.
pub(super) fn credit(p: &Palette) -> Div {
    div().flex_none().px(px(PADDING + 2.)).pt(px(4.)).flex_row_start().child(link(p))
}

fn link(p: &Palette) -> Stateful<Div> {
    let text = tr!("Subtitles provided by OpenSubtitles.com");
    let hover = p.hover;
    let link = div()
        .id("add-subtitles-opensubtitles-credit")
        .tab_index(0)
        .key_context(focus::CONTROL)
        .role(accesskit::Role::Link)
        .aria_label(text);
    focus::outline_ring(link, p)
        .min_h(px(MIN_TARGET))
        .flex()
        .items_center()
        .px(px(4.))
        .rounded(px(metrics::RADIUS))
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .text_size(text_px(metrics::TEXT_SMALL))
        .text_color(p.text_muted)
        .on_click(|_: &ClickEvent, _, cx| cx.open_url(SITE))
        .child(text)
}
