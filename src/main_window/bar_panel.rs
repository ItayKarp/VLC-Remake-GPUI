//! What the panels over the control bar's right end share: the track popover (S2.6) and the
//! settings panel (S2.7). Both are 400 px wide, 20 px from the window's right edge and above the
//! bar, as tall as designed but never above the title bar, and drawn on the same surface.

use gpui::{Div, Stateful, Window, div, prelude::*, px};

use super::TITLE_BAR_HEIGHT;
use crate::theme::Palette;
use crate::ui::metrics;

/// A panel's width in the designs.
const WIDTH: f32 = 400.;
/// Its distance from the window's right edge and from the bar.
const GAP: f32 = 20.;

/// A panel's surface, `design_height` tall where the window has room, above a control bar `bar`
/// tall: placed, sized and colored, with its children stacked top to bottom. It takes the
/// clicks over it.
pub(super) fn surface(
    id: &'static str,
    design_height: f32,
    bar: f32,
    window: &Window,
    p: &Palette,
) -> Stateful<Div> {
    let size = window.viewport_size();
    div()
        .id(id)
        .absolute()
        .right(px(GAP))
        .bottom(px(bar + GAP))
        .w(px(width(window)))
        .h(px(height(f32::from(size.height), design_height, bar)))
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(px(metrics::RADIUS_LARGE))
        .bg(p.panel)
        .text_color(p.text)
        .shadow_lg()
        .occlude()
}

/// A panel's width in `window`: as designed, narrower in a narrow window.
pub(super) fn width(window: &Window) -> f32 {
    WIDTH.min(f32::from(window.viewport_size().width) - 2. * GAP)
}

/// A panel's height in a window `window_height` tall over a control bar `bar` tall:
/// `design_height`, but never above the title bar (it scrolls instead).
fn height(window_height: f32, design_height: f32, bar: f32) -> f32 {
    let room = window_height - TITLE_BAR_HEIGHT - bar - 2. * GAP;
    room.clamp(0., design_height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_window_shortens_the_panel() {
        assert_eq!(height(1080., 524., 54.), 524.);
        assert_eq!(height(300., 524., 54.), 300. - 40. - 54. - 40.);
        assert_eq!(height(100., 524., 54.), 0.);
        assert_eq!(height(300., 524., 108.), 300. - 40. - 108. - 40.);
    }
}
