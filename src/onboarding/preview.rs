//! The mock player chrome around step 2's preview: a title bar and a control bar
//! drawn in the chosen theme and accent. Nothing here is interactive.

use gpui::{Div, IntoElement, Styled, Svg, div, prelude::*, px, relative, svg};

use crate::assets::icons;
use crate::i18n::DirectionalStyle;
use crate::theme::text_px;
use crate::theme::{Palette, blend};

/// How far the mock seek bar has played.
const SEEK_PROGRESS: f32 = 0.075;
/// How full the mock volume bar is.
const VOLUME_LEVEL: f32 = 0.35;

/// The preview's title bar: the small logo and the window buttons.
pub(super) fn title_bar(p: &Palette) -> impl IntoElement {
    div()
        .h(px(22.))
        .flex_row_start()
        .items_center()
        .justify_between()
        .px(px(8.))
        .bg(p.title_bar)
        .child(svg().path(icons::LOGO_SMALL).size(px(14.)).text_color(p.accent))
        .child(
            div().flex_row_start().gap(px(8.)).children(
                [icons::MINIMIZE, icons::MAXIMIZE, icons::FULLSCREEN, icons::CLOSE]
                    .map(|icon| svg().path(icon).size(px(12.)).text_color(p.icon)),
            ),
        )
}

/// The preview's playback controls, seek bar and volume.
pub(super) fn control_bar(p: &Palette) -> impl IntoElement {
    div()
        .h(px(29.))
        .flex_row_start()
        .items_center()
        .gap(px(10.))
        .px(px(14.))
        .bg(p.control_bar)
        .child(icon(icons::PREVIOUS, p))
        .child(play_button(p))
        .child(icon(icons::NEXT, p))
        .child(time("00:02:10", p))
        .child(seek_bar(p))
        .child(time("00:25:40", p))
        .child(icon(icons::VOLUME, p))
        .child(volume_bar(p))
        .child(icon(icons::AUDIO, p))
        .child(icon(icons::SUBTITLES, p))
        .child(icon(icons::SETTINGS, p))
        .child(icon(icons::LIST, p))
}

fn icon(path: &'static str, p: &Palette) -> Svg {
    svg().path(path).size(px(12.)).text_color(p.icon)
}

fn time(label: &'static str, p: &Palette) -> Div {
    div().text_size(text_px(8.)).text_color(p.text_muted).child(label)
}

fn play_button(p: &Palette) -> Div {
    div()
        .w(px(29.))
        .h(px(14.))
        .rounded(px(3.))
        .bg(p.accent)
        .flex_row_start()
        .items_center()
        .justify_center()
        .child(svg().path(icons::PLAY).size(px(11.)).text_color(p.on_accent))
}

fn seek_bar(p: &Palette) -> Div {
    div()
        .flex_1()
        .h(px(13.))
        .rounded(px(2.))
        .bg(p.sunken)
        .flex_row_start()
        .items_center()
        .child(
            div()
                .w(relative(SEEK_PROGRESS))
                .h_full()
                .rounded_l(px(2.))
                .bg(blend(p.accent, p.sunken, 0.23)),
        )
        .child(div().w(px(3.)).h(px(11.)).rounded(px(1.)).bg(p.accent))
}

fn volume_bar(p: &Palette) -> Div {
    div()
        .w(px(48.))
        .h(px(9.))
        .rounded(px(2.))
        .bg(p.sunken)
        .child(div().w(relative(VOLUME_LEVEL)).h_full().rounded(px(2.)).bg(p.volume))
}
