//! Icon helpers (S0.4.13). See `assets.rs` for the pipeline.

use gpui::{Div, Hsla, Svg, div, prelude::*, px, svg};

use crate::assets::TwoTone;

/// A single-color icon.
pub fn icon(path: &'static str, size: f32, color: Hsla) -> Svg {
    svg()
        .path(path)
        .size(px(size))
        .flex_none()
        .text_color(color)
}

/// A two-tone icon: the base layer in `base`, the accent layer on top in `accent`.
pub fn two_tone(icon: TwoTone, size: f32, base: Hsla, accent: Hsla) -> Div {
    div()
        .relative()
        .flex_none()
        .size(px(size))
        .child(
            svg()
                .absolute()
                .top_0()
                .left_0()
                .path(icon.base)
                .size(px(size))
                .text_color(base),
        )
        .child(
            svg()
                .absolute()
                .top_0()
                .left_0()
                .path(icon.accent)
                .size(px(size))
                .text_color(accent),
        )
}
