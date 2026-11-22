//! Turning wheel and trackpad scrolling into steps: the volume on the vertical axis (S2.5.5)
//! and seeking on the horizontal one (S2.10.1). Trackpad deltas add up until they make a step,
//! and each event makes one step at most, so an inertial flick ramps instead of jumping from
//! one end to the other.

use gpui::{Point, ScrollDelta, TouchPhase};

/// Pixels of trackpad scrolling per step.
const PIXELS_PER_STEP: f32 = 40.;

/// Which way a scroll goes along its axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Turn {
    /// Up, or left: louder, or back.
    Positive,
    /// Down, or right: quieter, or forward.
    Negative,
}

impl Turn {
    /// The other way when `reverse` ("Reverse scroll direction"), this way otherwise.
    pub fn reversed_if(self, reverse: bool) -> Self {
        match (self, reverse) {
            (turn, false) => turn,
            (Turn::Positive, true) => Turn::Negative,
            (Turn::Negative, true) => Turn::Positive,
        }
    }
}

/// One axis of scrolling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// Up and down.
    Vertical,
    /// Left and right.
    Horizontal,
}

impl Axis {
    /// The axis `delta` mostly scrolls along; vertical when it's even.
    pub fn of(delta: ScrollDelta) -> Self {
        let Point { x, y } = match delta {
            ScrollDelta::Lines(lines) => lines,
            ScrollDelta::Pixels(pixels) => pixels.map(f32::from),
        };
        if x.abs() > y.abs() { Axis::Horizontal } else { Axis::Vertical }
    }

    fn along(self, point: Point<f32>) -> f32 {
        match self {
            Axis::Vertical => point.y,
            Axis::Horizontal => point.x,
        }
    }
}

/// Scrolling along one axis, counted in steps.
#[derive(Clone, Copy, Debug)]
pub struct WheelSteps {
    axis: Axis,
    /// Scrolling not yet used, in steps; positive is up or left.
    pending: f32,
}

impl WheelSteps {
    /// Steps along `axis`.
    pub const fn new(axis: Axis) -> Self {
        Self { axis, pending: 0. }
    }

    /// The step `delta` makes along the axis, if any.
    pub fn take(&mut self, delta: ScrollDelta, phase: TouchPhase) -> Option<Turn> {
        if phase == TouchPhase::Started {
            self.pending = 0.;
        }
        let steps = match delta {
            // A wheel notch is a step, however many lines the system scrolls for it.
            ScrollDelta::Lines(lines) => signum_or_zero(self.axis.along(lines)),
            ScrollDelta::Pixels(pixels) => self.axis.along(pixels.map(f32::from)) / PIXELS_PER_STEP,
        };
        if steps.signum() != self.pending.signum() {
            // Turning back starts afresh.
            self.pending = 0.;
        }
        self.pending = (self.pending + steps).clamp(-1., 1.);
        if self.pending.abs() < 1. {
            return None;
        }
        let turn = if self.pending > 0. { Turn::Positive } else { Turn::Negative };
        self.pending = 0.;
        Some(turn)
    }
}

/// `value`'s sign, or 0 for 0 (`f32::signum` says 1 for +0).
fn signum_or_zero(value: f32) -> f32 {
    if value == 0. { 0. } else { value.signum() }
}

#[cfg(test)]
mod tests {
    use gpui::{point, px};

    use super::*;

    #[test]
    fn wheel_notches_step_once_each() {
        let mut wheel = WheelSteps::new(Axis::Vertical);
        let notch = |y| ScrollDelta::Lines(point(0., y));
        assert_eq!(wheel.take(notch(3.), TouchPhase::Moved), Some(Turn::Positive));
        assert_eq!(wheel.take(notch(-1.), TouchPhase::Moved), Some(Turn::Negative));
        assert_eq!(wheel.take(notch(0.), TouchPhase::Moved), None);
    }

    #[test]
    fn trackpad_scrolling_adds_up_and_never_jumps() {
        let mut wheel = WheelSteps::new(Axis::Vertical);
        let pixels = |y| ScrollDelta::Pixels(point(px(0.), px(y)));
        assert_eq!(wheel.take(pixels(15.), TouchPhase::Started), None);
        assert_eq!(wheel.take(pixels(30.), TouchPhase::Moved), Some(Turn::Positive));
        assert_eq!(wheel.take(pixels(400.), TouchPhase::Moved), Some(Turn::Positive));
        assert_eq!(wheel.take(pixels(-30.), TouchPhase::Moved), None, "turning back starts over");
        assert_eq!(wheel.take(pixels(-30.), TouchPhase::Moved), Some(Turn::Negative));
    }

    #[test]
    fn each_axis_counts_its_own_scrolling() {
        let mut sideways = WheelSteps::new(Axis::Horizontal);
        let notch = |x, y| ScrollDelta::Lines(point(x, y));
        assert_eq!(sideways.take(notch(0., 1.), TouchPhase::Moved), None);
        assert_eq!(sideways.take(notch(-1., 0.), TouchPhase::Moved), Some(Turn::Negative));
    }

    #[test]
    fn a_scroll_belongs_to_its_larger_axis() {
        let pixels = |x, y| ScrollDelta::Pixels(point(px(x), px(y)));
        assert_eq!(Axis::of(pixels(-30., 4.)), Axis::Horizontal);
        assert_eq!(Axis::of(pixels(3., -20.)), Axis::Vertical);
        assert_eq!(Axis::of(pixels(0., 0.)), Axis::Vertical);
    }
}
