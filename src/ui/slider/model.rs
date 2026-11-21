//! The slider's value and limits, free of any UI so the arithmetic is unit tested.

/// The value and its limits. Pure, so the arithmetic is unit tested.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderModel {
    /// The lowest value; never above `max`.
    pub min: f64,
    /// The highest value; never below `min`.
    pub max: f64,
    /// Arrow-key step; values snap to multiples of it from `min`. 0 disables
    /// snapping.
    pub step: f64,
    /// Shift+arrow and PageUp/PageDown.
    pub big_step: f64,
    /// Double-click restores this.
    pub default: f64,
    value: f64,
}

impl SliderModel {
    /// Creates a model at `default`. Reversed limits are swapped and the default is
    /// clamped into them.
    pub fn new(min: f64, max: f64, step: f64, default: f64) -> Self {
        let (min, max) = if min <= max { (min, max) } else { (max, min) };
        let mut model = Self {
            min,
            max,
            step: step.abs(),
            big_step: (step.abs() * 10.).min(max - min),
            default: default.clamp(min, max),
            value: min,
        };
        model.value = model.default;
        model
    }

    /// The current value, always within the limits.
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Sets the value, clamped and snapped. Returns whether it changed. NaN and
    /// infinite values are ignored.
    pub fn set(&mut self, value: f64) -> bool {
        if !value.is_finite() {
            return false;
        }
        let value = self.snap(value.clamp(self.min, self.max));
        // Exact on purpose: any difference at all is a change listeners must hear about.
        let changed = (value - self.value).abs() > 0.;
        self.value = value;
        changed
    }

    fn snap(&self, value: f64) -> f64 {
        if self.step <= 0. {
            return value;
        }
        let snapped = self.min + ((value - self.min) / self.step).round() * self.step;
        // Rounding can step past `max` when the range isn't a multiple of `step`.
        let snapped = snapped.clamp(self.min, self.max);
        // Keep 0.1 + 0.2 style noise out of displayed values.
        (snapped * 1e9).round() / 1e9
    }

    /// The value's position along the track, 0.0 to 1.0.
    pub fn fraction(&self) -> f64 {
        fraction_of(self.value, self.min, self.max)
    }

    /// Sets the value from a position along the track (0.0 to 1.0, clamped).
    /// Returns whether it changed.
    pub fn set_fraction(&mut self, fraction: f64) -> bool {
        if !fraction.is_finite() {
            return false;
        }
        self.set(self.min + fraction.clamp(0., 1.) * (self.max - self.min))
    }

    /// Moves the value by `steps` steps (big steps when `big`). Returns whether it
    /// changed.
    pub fn step_by(&mut self, steps: f64, big: bool) -> bool {
        let step = if big { self.big_step } else { self.step };
        let step = if step > 0. { step } else { (self.max - self.min) / 100. };
        self.set(self.value + steps * step)
    }

    /// The filled part of the track as (start, end) fractions. Centered sliders
    /// fill from zero (or the range's nearest end, when zero is outside it).
    pub fn fill(&self, centered: bool) -> (f64, f64) {
        let value = self.fraction();
        if !centered {
            return (0., value);
        }
        let origin = self.zero_fraction();
        (origin.min(value), origin.max(value))
    }

    /// Where zero sits along the track (the nearest end when it's outside the range).
    pub(super) fn zero_fraction(&self) -> f64 {
        fraction_of(0., self.min, self.max)
    }
}

fn fraction_of(value: f64, min: f64, max: f64) -> f64 {
    if max > min { ((value - min) / (max - min)).clamp(0., 1.) } else { 0. }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_and_snaps() {
        let mut m = SliderModel::new(0., 100., 5., 50.);
        assert_eq!(m.value(), 50.);
        assert!(m.set(42.));
        assert_eq!(m.value(), 40.);
        m.set(43.);
        assert_eq!(m.value(), 45.);
        m.set(1000.);
        assert_eq!(m.value(), 100.);
        m.set(-3.);
        assert_eq!(m.value(), 0.);
    }

    #[test]
    fn ignores_nan_and_infinity() {
        let mut m = SliderModel::new(0., 1., 0.1, 0.5);
        assert!(!m.set(f64::NAN));
        assert!(!m.set(f64::INFINITY));
        assert!(!m.set_fraction(f64::NAN));
        assert_eq!(m.value(), 0.5);
    }

    #[test]
    fn steps_without_float_noise() {
        let mut m = SliderModel::new(0., 1., 0.1, 0.);
        m.step_by(1., false);
        m.step_by(1., false);
        m.step_by(1., false);
        assert_eq!(m.value(), 0.3);
        // Big step is ten steps, clamped to the range.
        m.step_by(1., true);
        assert_eq!(m.value(), 1.);
    }

    #[test]
    fn snapping_never_passes_max() {
        // A range that isn't a multiple of the step.
        let mut m = SliderModel::new(0., 10., 3., 0.);
        m.set(9.9);
        assert_eq!(m.value(), 9.);
        m.set_fraction(1.);
        assert!(m.value() <= 10.);
    }

    #[test]
    fn reversed_limits_and_out_of_range_default() {
        let m = SliderModel::new(10., -10., 1., 50.);
        assert_eq!((m.min, m.max), (-10., 10.));
        assert_eq!(m.value(), 10.);
    }

    #[test]
    fn fraction_round_trip() {
        let mut m = SliderModel::new(-10., 10., 0., 0.);
        assert_eq!(m.fraction(), 0.5);
        m.set_fraction(0.75);
        assert_eq!(m.value(), 5.);
        // An empty range doesn't divide by zero.
        let empty = SliderModel::new(3., 3., 1., 3.);
        assert_eq!(empty.fraction(), 0.);
    }

    #[test]
    fn centered_fill_starts_at_zero() {
        let mut m = SliderModel::new(-12., 12., 1., 0.);
        m.set(6.);
        assert_eq!(m.fill(true), (0.5, 0.75));
        m.set(-6.);
        assert_eq!(m.fill(true), (0.25, 0.5));
        assert_eq!(m.fill(false), (0., 0.25));
        // Zero outside the range: fill from the nearest end.
        let mut positive = SliderModel::new(10., 20., 1., 15.);
        positive.set(15.);
        assert_eq!(positive.fill(true), (0., 0.5));
    }
}
