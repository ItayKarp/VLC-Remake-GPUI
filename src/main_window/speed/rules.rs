//! Playback speed as the user reads, types and steps it: `1.25x`, the `[`/`]` grid of
//! quarter steps, and the forms the speed field accepts (S2.7 edge cases).

use crate::engine::values::{MAX_RATE, MIN_RATE};
use crate::ui::slider::parse_number;

/// The grid `[` and `]` step along, and the slider's ticks: a quarter of normal speed.
pub const STEP: f32 = 0.25;
/// Leeway when deciding whether a speed is on the grid, for float noise.
const EPSILON: f32 = 1e-4;

/// Typed text that isn't a speed.
#[derive(Debug, PartialEq, Eq)]
pub struct Invalid;

/// Which way a key moves the speed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeedChange {
    /// Up to the next quarter step.
    Faster,
    /// Down to the previous quarter step.
    Slower,
    /// Back to normal speed.
    Normal,
}

/// The speed after `change` from `speed`, on the quarter-step grid and in range. A speed off the
/// grid (typed `1.1`) goes to the nearest step in that direction.
pub fn changed(speed: f32, change: SpeedChange) -> f32 {
    let steps = speed / STEP;
    let next = match change {
        SpeedChange::Faster => ((steps + EPSILON).floor() + 1.) * STEP,
        SpeedChange::Slower => ((steps - EPSILON).ceil() - 1.) * STEP,
        SpeedChange::Normal => 1.,
    };
    next.clamp(MIN_RATE, MAX_RATE)
}

/// Reads a typed speed: `1.5`, `1,5`, `1.5x`, `1.5×` or `150%`, clamped to the range the player
/// allows.
pub fn parse(text: &str) -> Result<f32, Invalid> {
    let text = text.trim();
    let (text, scale) = match text.strip_suffix('%') {
        Some(percent) => (percent, 100.),
        None => (text, 1.),
    };
    let value = parse_number(text).ok_or(Invalid)? / scale;
    if value <= 0. {
        return Err(Invalid);
    }
    Ok((value as f32).clamp(MIN_RATE, MAX_RATE))
}

/// The speed as a number without trailing zeros: `1`, `1.5`, `0.25`.
pub fn number(speed: f64) -> String {
    let hundredths = (speed * 100.).round() as i64;
    let digits = match hundredths {
        h if h % 100 == 0 => 0,
        h if h % 10 == 0 => 1,
        _ => 2,
    };
    crate::i18n::decimal(speed, digits)
}

/// The speed for the field and the slider's bubble: `1.25x`, as designed.
pub fn field_text(speed: f64) -> String {
    format!("{}x", number(speed))
}

/// The speed for the on-screen message: `Speed 1.25×`.
pub fn message(speed: f32) -> String {
    tr!("Speed {speed}×", speed = number(f64::from(speed)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_step_along_the_quarter_grid() {
        assert_eq!(changed(1., SpeedChange::Faster), 1.25);
        assert_eq!(changed(1., SpeedChange::Slower), 0.75);
        assert_eq!(changed(1.1, SpeedChange::Faster), 1.25);
        assert_eq!(changed(1.1, SpeedChange::Slower), 1.);
        assert_eq!(changed(2.5, SpeedChange::Normal), 1.);
    }

    #[test]
    fn keys_stop_at_the_ends() {
        assert_eq!(changed(MAX_RATE, SpeedChange::Faster), MAX_RATE);
        assert_eq!(changed(MIN_RATE, SpeedChange::Slower), MIN_RATE);
    }

    #[test]
    fn every_typed_form_is_read() {
        for text in ["1.5", "1,5", "1.5x", "1.5×", " 1.5 x ", "150%", "150 %"] {
            assert_eq!(parse(text), Ok(1.5), "{text}");
        }
    }

    #[test]
    fn typed_speeds_outside_the_range_are_clamped() {
        assert_eq!(parse("0.1"), Ok(MIN_RATE));
        assert_eq!(parse("8x"), Ok(MAX_RATE));
    }

    #[test]
    fn nonsense_is_invalid() {
        for text in ["", "fast", "x", "0", "-1", "%"] {
            assert_eq!(parse(text), Err(Invalid), "{text}");
        }
    }

    #[test]
    fn speeds_read_without_trailing_zeros() {
        assert_eq!(field_text(1.), "1x");
        assert_eq!(field_text(0.25), "0.25x");
        assert_eq!(field_text(1.5), "1.5x");
        assert_eq!(message(1.25), "Speed 1.25×");
    }
}
