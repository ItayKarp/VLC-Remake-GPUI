//! The video viewport (S2.1.4): the video inset from the window's edges in a rounded rectangle,
//! letterboxed inside it. Under the title bar the inset leaves the top edge; with the bars
//! floating or hidden (S2.8) it goes all round. Fullscreen shows the video edge to edge.
//!
//! Video frames are painted as images or (macOS) surfaces, which GPUI's rounded content masks
//! don't clip, so the corners are rounded by painting the frame's colour over them.

use gpui::{
    Bounds, Div, Entity, Hsla, PathBuilder, Pixels, Point, Window, canvas, div, point, prelude::*,
    px,
};

use super::auto_hide::rules::Layout;
use crate::theme::Palette;
use crate::video::VideoView;

/// How far the viewport sits from the window's sides and bottom (`player_01.png`).
const INSET: f32 = 10.;
/// The viewport's corner radius.
const RADIUS: f32 = 6.;
/// How far a quarter circle's Bézier control points sit along its tangents, per unit radius.
const QUARTER_CIRCLE: f32 = 0.552_284_8;

/// `video` in its rounded viewport for `layout`, framed in the title bar's colour; edge to edge
/// in fullscreen.
pub fn framed(video: Entity<VideoView>, p: &Palette, layout: Layout) -> Div {
    let frame = p.title_bar;
    let area = div().size_full().bg(frame);
    let area = match layout {
        Layout::Fullscreen => return div().size_full().child(video),
        Layout::Docked => area.px(px(INSET)).pb(px(INSET)),
        Layout::Floating => area.p(px(INSET)),
    };
    area.child(
        div().relative().size_full().rounded(px(RADIUS)).bg(p.video).child(video).child(
            canvas(|_, _, _| {}, move |bounds, (), window, _| round_corners(bounds, frame, window))
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
        ),
    )
}

/// Paints `color` outside a quarter circle in each corner of `bounds`.
fn round_corners(bounds: Bounds<Pixels>, color: Hsla, window: &mut Window) {
    let r = px(RADIUS);
    let corners = [
        (bounds.origin, r, r),
        (bounds.top_right(), -r, r),
        (bounds.bottom_left(), r, -r),
        (bounds.bottom_right(), -r, -r),
    ];
    for (corner, dx, dy) in corners {
        let mut cap = PathBuilder::fill();
        let along_x = point(corner.x + dx, corner.y);
        let along_y = point(corner.x, corner.y + dy);
        cap.move_to(corner);
        cap.line_to(along_x);
        cap.cubic_bezier_to(
            along_y,
            toward(along_x, corner, QUARTER_CIRCLE),
            toward(along_y, corner, QUARTER_CIRCLE),
        );
        cap.line_to(corner);
        match cap.build() {
            Ok(path) => window.paint_path(path, color),
            Err(error) => tracing::warn!("viewport: couldn't build a corner: {error:#}"),
        }
    }
}

/// The point `fraction` of the way from `from` to `to`.
fn toward(from: Point<Pixels>, to: Point<Pixels>, fraction: f32) -> Point<Pixels> {
    point(from.x + (to.x - from.x) * fraction, from.y + (to.y - from.y) * fraction)
}
