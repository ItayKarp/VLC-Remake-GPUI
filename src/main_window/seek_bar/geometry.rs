//! The seek bar's arithmetic, kept apart from drawing so it can be tested: where on the track
//! a pointer is, scrubbing (with Shift for fine scrubbing, S2.3 edge cases), where the hover
//! card goes, and how far the buffered band reaches.

use std::time::Duration;

/// How much slower the handle follows the pointer while Shift is held.
pub const FINE_SCRUB: f32 = 0.1;

/// The track's horizontal span, in window pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    /// Its left edge.
    pub left: f32,
    /// Its width; a zero width is treated as one pixel.
    pub width: f32,
}

impl Span {
    /// The fraction of the track at window position `x`, clamped to 0–1.
    pub fn fraction_at(self, x: f32) -> f32 {
        ((x - self.left) / self.width.max(1.)).clamp(0., 1.)
    }

    /// The window position of `fraction` along the track.
    pub fn x_of(self, fraction: f32) -> f32 {
        self.left + self.width * fraction.clamp(0., 1.)
    }
}

/// A drag along the track. The handle follows the pointer, or a tenth as fast while Shift is
/// held; pressing or releasing Shift mid-drag continues from where the handle is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scrub {
    fraction: f32,
    anchor_x: f32,
    anchor_fraction: f32,
    fine: bool,
}

impl Scrub {
    /// A drag starting at window position `x`: the handle jumps there.
    pub fn start(x: f32, span: Span) -> Self {
        let fraction = span.fraction_at(x);
        Self { fraction, anchor_x: x, anchor_fraction: fraction, fine: false }
    }

    /// The pointer moved to `x`, with Shift held when `fine`. Positions outside the track (or
    /// the window) clamp to its ends.
    pub fn move_to(&mut self, x: f32, fine: bool, span: Span) {
        if fine != self.fine {
            // Re-anchor at the handle, so switching speeds never makes it jump.
            self.anchor_x = x;
            self.anchor_fraction = self.fraction;
            self.fine = fine;
        }
        let speed = if fine { FINE_SCRUB } else { 1. };
        let moved = (x - self.anchor_x) / span.width.max(1.) * speed;
        self.fraction = (self.anchor_fraction + moved).clamp(0., 1.);
    }

    /// Where the handle is, 0–1.
    pub fn fraction(self) -> f32 {
        self.fraction
    }
}

/// The time at `fraction` of `duration`. Never divides, so even a zero-length or sub-second
/// media maps cleanly.
pub fn time_at(fraction: f32, duration: Duration) -> Duration {
    duration.mul_f64(f64::from(fraction.clamp(0., 1.)))
}

/// How far through `duration` the time `position` is, 0–1; 0 for an empty duration.
pub fn fraction_of(position: Duration, duration: Duration) -> f32 {
    if duration.is_zero() {
        return 0.;
    }
    (position.as_secs_f64() / duration.as_secs_f64()).clamp(0., 1.) as f32
}

/// The left edge of a `card_width` card centred on `x`, kept `margin` inside a window
/// `window_width` wide (S2.3.2). A window narrower than the card puts it at the margin.
pub fn card_left(x: f32, card_width: f32, window_width: f32, margin: f32) -> f32 {
    let max_left = (window_width - card_width - margin).max(margin);
    (x - card_width / 2.).clamp(margin, max_left)
}

/// libVLC's default network cache, which its buffering level is a percentage of.
pub const NETWORK_CACHE: Duration = Duration::from_secs(1);

/// Where the buffered band ends (S2.3.5), 0–1: libVLC 3 reports how full its input cache is,
/// not which ranges are downloaded, so the band runs from the position over the cached part of
/// the [`NETWORK_CACHE`].
pub fn buffered_end(position: Duration, fill_percent: f32, duration: Duration) -> f32 {
    let cached = NETWORK_CACHE.mul_f32(fill_percent.clamp(0., 100.) / 100.);
    fraction_of(position + cached, duration)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPAN: Span = Span { left: 100., width: 1000. };

    #[test]
    fn pointer_positions_map_to_the_track_and_clamp() {
        assert!((SPAN.fraction_at(600.) - 0.5).abs() < f32::EPSILON);
        assert!(SPAN.fraction_at(-50.).abs() < f32::EPSILON);
        assert!((SPAN.fraction_at(5000.) - 1.).abs() < f32::EPSILON);
        assert!((SPAN.x_of(0.25) - 350.).abs() < f32::EPSILON);
    }

    #[test]
    fn a_zero_width_track_does_not_divide_by_zero() {
        let span = Span { left: 10., width: 0. };
        assert!(span.fraction_at(10.5).is_finite());
    }

    #[test]
    fn dragging_follows_the_pointer_and_clamps_outside_the_window() {
        let mut scrub = Scrub::start(600., SPAN);
        scrub.move_to(700., false, SPAN);
        assert!((scrub.fraction() - 0.6).abs() < 1e-6);
        scrub.move_to(-400., false, SPAN);
        assert!(scrub.fraction().abs() < f32::EPSILON);
    }

    #[test]
    fn shift_scrubs_ten_times_finer_without_jumping() {
        let mut scrub = Scrub::start(600., SPAN);
        scrub.move_to(600., true, SPAN);
        assert!((scrub.fraction() - 0.5).abs() < 1e-6, "pressing Shift doesn't move it");
        scrub.move_to(700., true, SPAN);
        assert!((scrub.fraction() - 0.51).abs() < 1e-6);
        scrub.move_to(700., false, SPAN);
        assert!((scrub.fraction() - 0.51).abs() < 1e-6, "releasing Shift doesn't move it");
        scrub.move_to(800., false, SPAN);
        assert!((scrub.fraction() - 0.61).abs() < 1e-6);
    }

    #[test]
    fn very_short_and_empty_media_map_without_dividing() {
        assert_eq!(time_at(0.5, Duration::from_millis(400)), Duration::from_millis(200));
        assert_eq!(time_at(0.5, Duration::ZERO), Duration::ZERO);
        assert!(fraction_of(Duration::from_secs(1), Duration::ZERO).abs() < f32::EPSILON);
    }

    #[test]
    fn the_card_stays_inside_the_window() {
        assert!((card_left(500., 200., 1000., 8.) - 400.).abs() < f32::EPSILON);
        assert!((card_left(20., 200., 1000., 8.) - 8.).abs() < f32::EPSILON);
        assert!((card_left(990., 200., 1000., 8.) - 792.).abs() < f32::EPSILON);
        assert!((card_left(50., 200., 150., 8.) - 8.).abs() < f32::EPSILON);
    }

    #[test]
    fn the_buffered_band_covers_the_filled_cache() {
        let end = buffered_end(Duration::from_secs(5), 50., Duration::from_secs(10));
        assert!((end - 0.55).abs() < 1e-6);
        let end = buffered_end(Duration::from_secs(10), 100., Duration::from_secs(10));
        assert!((end - 1.).abs() < f32::EPSILON);
    }
}
