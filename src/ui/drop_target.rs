//! The highlight a window shows while files are dragged over it (S1.3.2).
//!
//! The window's root takes the drop and names itself with [`GROUP`]; [`highlight`] is a
//! frame drawn over everything, transparent until a file drag is over the window. It sits on
//! top without taking clicks, so it works the same over menus, popovers and in fullscreen.

use gpui::{Div, ExternalPaths, div, prelude::*, px};

use crate::theme::Palette;

/// The group name the drop-target root sets with `.group(GROUP)`.
pub const GROUP: &str = "drop-target";

/// The frame's width.
const BORDER: f32 = 3.;

/// The accent frame and tint over the whole window while files are dragged over it. Add it as
/// the root's last child.
pub fn highlight(p: &Palette) -> Div {
    let accent = p.accent_fg;
    div()
        .absolute()
        .inset_0()
        .border(px(BORDER))
        .border_color(gpui::transparent_black())
        .group_drag_over::<ExternalPaths>(GROUP, move |style| {
        style.border_color(accent).bg(accent.opacity(0.08))
    })
}
