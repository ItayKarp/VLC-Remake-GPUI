//! Popovers anchored to a trigger (S0.4.6), and the surface menus, dropdowns and
//! tooltips share.
//!
//! The owner keeps a [`BoundsTracker`] inside the trigger, and renders
//! [`attached`] while open. GPUI's `anchored().attach` places the popover next to
//! the trigger's bounds from the previous frame, moves it to the other side when
//! that fits better (below → above, right → left), and shifts it inside the window.
//! The surface's height is capped at the window's, so a popover that still
//! doesn't fit (a tiny window) scrolls.
//!
//! Because the position is recomputed every frame from the tracked bounds, a
//! popover follows its trigger when the window resizes or moves to another
//! monitor.
//!
//! Owners close popovers on outside click (`on_mouse_down_out` on the surface),
//! Escape, and window deactivation ([`close_on_deactivate`]).

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    Anchor, AnyElement, Bounds, Context, Div, Pixels, Point, Window, anchored, canvas, deferred,
    div, point, prelude::*, px,
};

use super::metrics;
use crate::theme::Palette;

/// Records an element's bounds every frame. Put [`Self::element`] inside the
/// element to track (which must be `relative`).
#[derive(Clone, Default)]
pub struct BoundsTracker(Rc<Cell<Option<Bounds<Pixels>>>>);

impl BoundsTracker {
    /// A tracker that hasn't seen a frame yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// The bounds from the last frame, or `None` before the first one.
    pub fn bounds(&self) -> Option<Bounds<Pixels>> {
        self.0.get()
    }

    /// Whether `position` was inside the element last frame.
    #[allow(
        clippy::trivially_copy_pass_by_ref,
        reason = "dropdowns and the gallery call this with `&event.position`; they are \
                  outside this change"
    )]
    pub fn contains(&self, position: &Point<Pixels>) -> bool {
        self.bounds().is_some_and(|b| b.contains(position))
    }

    /// An invisible element covering its parent that records the parent's bounds.
    pub fn element(&self) -> impl IntoElement {
        let cell = self.0.clone();
        canvas(move |bounds, _, _| cell.set(Some(bounds)), |_, (), _, _| {})
            .absolute()
            .top_0()
            .left_0()
            .size_full()
    }
}

/// Which side of the trigger a popover prefers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// Below, left edges aligned (dropdowns, title bar menus).
    Below,
    /// Above, right edges aligned (control bar popovers at the bottom right).
    #[allow(dead_code, reason = "the control bar uses it (Stage 2)")]
    AboveEnd,
    /// Above, centered (control bar popovers).
    Above,
    /// To the right, top edges aligned (submenus).
    Right,
}

impl Side {
    /// The popover corner and the trigger corner that touch, and the gap's
    /// direction.
    pub fn corners(self) -> (Anchor, Anchor, Point<f32>) {
        match self {
            Side::Below => (Anchor::TopLeft, Anchor::BottomLeft, point(0., 1.)),
            Side::AboveEnd => (Anchor::BottomRight, Anchor::TopRight, point(0., -1.)),
            Side::Above => (Anchor::BottomCenter, Anchor::TopCenter, point(0., -1.)),
            Side::Right => (Anchor::TopLeft, Anchor::TopRight, point(1., 0.)),
        }
    }
}

/// A floating layer next to `trigger`, painted above the rest of the window.
/// `priority` orders nested layers (a submenu above its menu).
pub fn attached(
    trigger: Bounds<Pixels>,
    side: Side,
    gap: f32,
    priority: usize,
    content: impl IntoElement,
) -> AnyElement {
    let (anchor, corner, direction) = side.corners();
    deferred(
        anchored()
            .attach(trigger, corner)
            .anchor(anchor)
            .offset(point(px(direction.x * gap), px(direction.y * gap)))
            .snap_to_window_with_margin(px(metrics::EDGE_MARGIN))
            .child(content),
    )
    .with_priority(priority)
    .into_any_element()
}

/// A floating layer with its top-left corner at `position` (a context menu at the
/// pointer). It moves to the pointer's other side when it would leave the window.
pub fn at_position(
    position: Point<Pixels>,
    priority: usize,
    content: impl IntoElement,
) -> AnyElement {
    deferred(
        anchored()
            .position(position)
            .anchor(Anchor::TopLeft)
            .snap_to_window_with_margin(px(metrics::EDGE_MARGIN))
            .child(content),
    )
    .with_priority(priority)
    .into_any_element()
}

/// The tallest a floating surface may be in this window: it scrolls beyond that.
pub fn max_height(window: &Window) -> Pixels {
    max_height_for(window.viewport_size().height)
}

/// The tallest a floating surface may be in a viewport this tall: the viewport less
/// its edge margins, but never less than a usable minimum.
pub fn max_height_for(viewport_height: Pixels) -> Pixels {
    (viewport_height - px(2. * metrics::EDGE_MARGIN)).max(px(48.))
}

/// The ~95% opaque surface menus, dropdowns and popovers use (`library/menus/*`,
/// `dropdown-open`): rounded, bordered, with a drop shadow. Blocks the mouse from
/// what's underneath.
pub fn surface(p: &Palette, window: &Window) -> Div {
    div()
        .occlude()
        .max_h(max_height(window))
        .rounded(px(metrics::RADIUS_LARGE))
        .bg(p.menu)
        .border_1()
        .border_color(p.divider)
        .shadow_lg()
        .text_color(p.text)
}

/// Closes a popover when its window loses focus (another app or window comes to
/// the front).
pub fn close_on_deactivate<V: 'static>(
    window: &mut Window,
    cx: &mut Context<V>,
    close: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) {
    cx.observe_window_activation(window, move |this, window, cx| {
        if !window.is_window_active() {
            close(this, window, cx);
        }
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_height_leaves_margins_and_has_a_floor() {
        assert_eq!(max_height_for(px(600.)), px(584.));
        assert_eq!(max_height_for(px(40.)), px(48.));
    }

    #[test]
    fn sides_touch_the_trigger() {
        // Below: the popover's top-left sits on the trigger's bottom-left.
        assert_eq!(Side::Below.corners(), (Anchor::TopLeft, Anchor::BottomLeft, point(0., 1.)));
        // Submenus open to the right, top edges aligned.
        assert_eq!(Side::Right.corners().1, Anchor::TopRight);
    }

    #[test]
    fn tracker_starts_empty() {
        let tracker = BoundsTracker::new();
        assert!(tracker.bounds().is_none());
        assert!(!tracker.contains(&point(px(1.), px(1.))));
    }
}
