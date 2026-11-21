//! The volume slider's arithmetic, kept apart from drawing so it can be tested: the non-linear
//! mapping between volumes and track positions (S2.5.2), the magnet at 100% (S2.5.6), the
//! position-locked colours (S2.5.1), and turning wheel deltas into steps (S2.5.5).

use gpui::{Hsla, Rgba, ScrollDelta, TouchPhase, rgb};

use crate::engine::values::{Volume, VolumeChange};

/// The share of the track 0–100% takes when the maximum is 200%, measured from the designs.
const FULL_SHARE_AT_200: f32 = 0.71;
/// How long each 100% of boost is on the track, relative to 0–100%.
const BOOST_LENGTH: f32 = (1. - FULL_SHARE_AT_200) / FULL_SHARE_AT_200;
/// How close to 100% a drag holds at 100% (S2.5.6), as a level.
const DETENT: f32 = 0.03;

/// The fill's green at silence.
const GREEN: u32 = 0x78A613;
/// The amber at 100%.
const AMBER: u32 = 0x9A7513;
/// The red at 200% and above.
const RED: u32 = 0xC93129;
/// The gradient's colours and the levels they sit at: in between, colours blend.
const STOPS: [(f32, u32); 3] = [(0., GREEN), (1., AMBER), (2., RED)];

/// Pixels of trackpad scrolling per volume step.
const PIXELS_PER_STEP: f32 = 40.;

/// The track's scale for one "Maximum volume": 0–100% is a linear part, and every 100% of
/// boost after it a shorter linear part, so 0–100% takes 71% of a 200% track, all of a 100%
/// track and 55% of a 300% one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scale {
    /// The loudest level, at least 1.0.
    max: f32,
}

impl Scale {
    /// The scale of a track that ends at `max`.
    pub fn new(max: Volume) -> Self {
        Self { max: max.get().max(1.) }
    }

    /// The track's length, in lengths of its 0–100% part.
    fn length(self) -> f32 {
        1. + (self.max - 1.) * BOOST_LENGTH
    }

    /// Where `volume` sits along the track, 0–1.
    pub fn fraction_of(self, volume: Volume) -> f32 {
        let level = volume.get().min(self.max);
        let along = if level <= 1. { level } else { 1. + (level - 1.) * BOOST_LENGTH };
        along / self.length()
    }

    /// The volume at `fraction` of the track, clamped to it.
    pub fn volume_at(self, fraction: f32) -> Volume {
        let along = fraction.clamp(0., 1.) * self.length();
        let level = if along <= 1. { along } else { 1. + (along - 1.) / BOOST_LENGTH };
        Volume::new(level.min(self.max))
    }

    /// The volume a drag to `fraction` sets: a whole percent, held at 100% within the detent so
    /// a drag doesn't drift into the boost by accident.
    pub fn dragged_to(self, fraction: f32) -> Volume {
        let volume = self.volume_at(fraction);
        if (volume.get() - 1.).abs() <= DETENT {
            return Volume::FULL;
        }
        Volume::from_percent(volume.as_percent())
    }

    /// The fill up to `volume`, split where the gradient changes direction: each part's track
    /// fractions and its colours at both ends.
    pub fn segments(self, volume: Volume) -> Vec<Segment> {
        let level = volume.get().min(self.max);
        let mut edges = vec![0.];
        edges.extend(STOPS.iter().map(|(at, _)| *at).filter(|at| *at > 0. && *at < level));
        edges.push(level);
        edges
            .windows(2)
            .filter(|pair| pair[1] > pair[0])
            .map(|pair| Segment {
                from: self.fraction_of(Volume::new(pair[0])),
                to: self.fraction_of(Volume::new(pair[1])),
                start: color_at(pair[0]),
                end: color_at(pair[1]),
            })
            .collect()
    }
}

/// One straight stretch of the fill's gradient.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    /// Where it starts along the track, 0–1.
    pub from: f32,
    /// Where it ends along the track, 0–1.
    pub to: f32,
    /// Its colour at `from`.
    pub start: Hsla,
    /// Its colour at `to`.
    pub end: Hsla,
}

/// The fill's colour at `level`: it belongs to the level, not to how far the fill reaches.
pub fn color_at(level: f32) -> Hsla {
    let level = level.max(0.);
    let next = STOPS.iter().position(|(at, _)| *at > level);
    let Some(next) = next.filter(|next| *next > 0) else {
        return rgb(STOPS[STOPS.len() - 1].1).into();
    };
    let ((low, from), (high, to)) = (STOPS[next - 1], STOPS[next]);
    mix(rgb(from), rgb(to), (level - low) / (high - low)).into()
}

/// The colour `amount` of the way from `from` to `to`.
fn mix(from: Rgba, to: Rgba, amount: f32) -> Rgba {
    let mix = |a: f32, b: f32| a + (b - a) * amount;
    Rgba { r: mix(from.r, to.r), g: mix(from.g, to.g), b: mix(from.b, to.b), a: 1. }
}

/// Turns wheel and trackpad scrolling into volume steps (S2.5.5). Trackpad deltas add up until
/// they make a step, and each event makes one step at most, so an inertial flick ramps the
/// volume instead of jumping from 0 to the maximum.
#[derive(Clone, Copy, Debug, Default)]
pub struct WheelSteps {
    /// Scrolling not yet used, in steps; positive is louder.
    pending: f32,
}

impl WheelSteps {
    /// The step `delta` makes, if any. Scrolling up is louder.
    pub fn take(&mut self, delta: ScrollDelta, phase: TouchPhase) -> Option<VolumeChange> {
        if phase == TouchPhase::Started {
            self.pending = 0.;
        }
        let steps = match delta {
            ScrollDelta::Lines(lines) if lines.y == 0. => 0.,
            // A wheel notch is a step, however many lines the system scrolls for it.
            ScrollDelta::Lines(lines) => lines.y.signum(),
            ScrollDelta::Pixels(pixels) => f32::from(pixels.y) / PIXELS_PER_STEP,
        };
        if steps.signum() != self.pending.signum() {
            // Turning back starts afresh.
            self.pending = 0.;
        }
        self.pending = (self.pending + steps).clamp(-1., 1.);
        if self.pending.abs() < 1. {
            return None;
        }
        let change = if self.pending > 0. { VolumeChange::Up } else { VolumeChange::Down };
        self.pending = 0.;
        Some(change)
    }
}

#[cfg(test)]
mod tests {
    use gpui::{point, px};

    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn full_volume_takes_71_percent_of_a_200_percent_track() {
        let scale = Scale::new(Volume::from_percent(200));
        assert!(close(scale.fraction_of(Volume::FULL), 0.71));
        assert!(close(scale.fraction_of(Volume::from_percent(50)), 0.355));
        assert!(close(scale.fraction_of(Volume::from_percent(150)), 0.855));
        assert!(close(scale.fraction_of(Volume::from_percent(200)), 1.));
    }

    #[test]
    fn the_scale_extends_to_other_maximums() {
        let plain = Scale::new(Volume::FULL);
        assert!(close(plain.fraction_of(Volume::from_percent(50)), 0.5));
        let loud = Scale::new(Volume::from_percent(300));
        let full = loud.fraction_of(Volume::FULL);
        assert!(close(full, 1. / (1. + 2. * BOOST_LENGTH)));
        assert!(close(loud.fraction_of(Volume::from_percent(300)), 1.));
    }

    #[test]
    fn positions_and_volumes_map_both_ways() {
        let scale = Scale::new(Volume::from_percent(200));
        for percent in [0, 30, 100, 140, 200] {
            let volume = Volume::from_percent(percent);
            assert_eq!(scale.volume_at(scale.fraction_of(volume)).as_percent(), percent);
        }
        assert_eq!(scale.volume_at(1.5).as_percent(), 200);
        assert_eq!(scale.volume_at(-1.).as_percent(), 0);
    }

    #[test]
    fn a_drag_near_100_percent_holds_there() {
        let scale = Scale::new(Volume::from_percent(200));
        let at = |percent| scale.fraction_of(Volume::from_percent(percent));
        assert_eq!(scale.dragged_to(at(103)), Volume::FULL);
        assert_eq!(scale.dragged_to(at(97)), Volume::FULL);
        assert_eq!(scale.dragged_to(at(105)).as_percent(), 105);
        assert_eq!(scale.dragged_to(at(90)).as_percent(), 90);
    }

    #[test]
    fn colours_are_locked_to_levels() {
        assert_eq!(color_at(0.), rgb(GREEN).into());
        assert_eq!(color_at(1.), rgb(AMBER).into());
        assert_eq!(color_at(2.), rgb(RED).into());
        assert_eq!(color_at(2.7), rgb(RED).into());
    }

    #[test]
    fn the_fill_splits_at_the_gradient_stops() {
        let scale = Scale::new(Volume::from_percent(200));
        assert_eq!(scale.segments(Volume::from_percent(50)).len(), 1);
        let boosted = scale.segments(Volume::from_percent(150));
        assert_eq!(boosted.len(), 2);
        assert!(close(boosted[0].to, 0.71));
        assert_eq!(boosted[1].start, rgb(AMBER).into());
        assert_eq!(scale.segments(Volume::new(0.)).len(), 0);
    }

    #[test]
    fn wheel_notches_step_once_each() {
        let mut wheel = WheelSteps::default();
        let notch = |y| ScrollDelta::Lines(point(0., y));
        assert_eq!(wheel.take(notch(3.), TouchPhase::Moved), Some(VolumeChange::Up));
        assert_eq!(wheel.take(notch(-1.), TouchPhase::Moved), Some(VolumeChange::Down));
        assert_eq!(wheel.take(notch(0.), TouchPhase::Moved), None);
    }

    #[test]
    fn trackpad_scrolling_adds_up_and_never_jumps() {
        let mut wheel = WheelSteps::default();
        let pixels = |y| ScrollDelta::Pixels(point(px(0.), px(y)));
        assert_eq!(wheel.take(pixels(15.), TouchPhase::Started), None);
        assert_eq!(wheel.take(pixels(30.), TouchPhase::Moved), Some(VolumeChange::Up));
        assert_eq!(wheel.take(pixels(400.), TouchPhase::Moved), Some(VolumeChange::Up));
        assert_eq!(wheel.take(pixels(-30.), TouchPhase::Moved), None, "turning back starts over");
        assert_eq!(wheel.take(pixels(-30.), TouchPhase::Moved), Some(VolumeChange::Down));
    }
}
