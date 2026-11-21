//! Window sizing and placement maths (S0.2.7, S0.2.9), kept free of GPUI so it can be
//! unit-tested. Everything is in logical pixels in GPUI's global coordinate space.

use gpui::{Bounds, Pixels, WindowBounds, point, px, size};

use crate::settings::{WindowGeometry, WindowMode};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Rect { x, y, w, h }
    }

    fn intersection_area(&self, other: &Rect) -> f32 {
        let w = (self.x + self.w).min(other.x + other.w) - self.x.max(other.x);
        let h = (self.y + self.h).min(other.y + other.h) - self.y.max(other.y);
        if w > 0. && h > 0. { w * h } else { 0. }
    }

    pub fn center(&self) -> (f32, f32) {
        (self.x + self.w / 2., self.y + self.h / 2.)
    }

    pub fn contains(&self, (x, y): (f32, f32)) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }

    pub fn centered_in(w: f32, h: f32, area: &Rect) -> Rect {
        Rect::new(area.x + (area.w - w) / 2., area.y + (area.h - h) / 2., w, h)
    }

    pub fn from_gpui(bounds: Bounds<Pixels>) -> Self {
        Rect::new(
            f32::from(bounds.origin.x),
            f32::from(bounds.origin.y),
            f32::from(bounds.size.width),
            f32::from(bounds.size.height),
        )
    }

    pub fn to_gpui(self) -> Bounds<Pixels> {
        Bounds::new(point(px(self.x), px(self.y)), size(px(self.w), px(self.h)))
    }
}

/// The size a fixed-layout window gets on a display: the design size, shrunk to fit
/// the display's visible area (S0.2.7). Returns `(window size, minimum size)`; the
/// window only needs to be resizable when it was shrunk.
pub fn fit_to_display(
    design: (f32, f32),
    min: (f32, f32),
    visible: &Rect,
) -> ((f32, f32), (f32, f32)) {
    let w = design.0.min(visible.w);
    let h = design.1.min(visible.h);
    ((w, h), (min.0.min(w), min.1.min(h)))
}

/// Where a saved window should reopen. `displays` are the visible bounds of the
/// connected displays; `fallback` is the display to use when the saved position is
/// on none of them (monitor unplugged, resolution changed). The result always lies
/// within one display and is at least `min` (or the whole display, if smaller).
/// `None` if nothing valid was saved.
pub fn restore(
    saved: &WindowGeometry,
    displays: &[Rect],
    fallback: &Rect,
    min: (f32, f32),
) -> Option<WindowBounds> {
    let valid = |v: f32| v.is_finite();
    if !(valid(saved.x) && valid(saved.y) && saved.width > 0. && saved.height > 0.)
        || !(valid(saved.width) && valid(saved.height))
    {
        return None;
    }
    let rect = Rect::new(saved.x, saved.y, saved.width, saved.height);
    let display = displays
        .iter()
        .map(|d| (d, rect.intersection_area(d)))
        .filter(|(_, area)| *area > 0.)
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(d, _)| *d);
    let placed = match display {
        Some(display) => clamp_into(rect, &display, min),
        None => {
            let clamped = clamp_into(rect, fallback, min);
            Rect::centered_in(clamped.w, clamped.h, fallback)
        }
    };
    let bounds = placed.to_gpui();
    Some(match saved.mode {
        WindowMode::Windowed => WindowBounds::Windowed(bounds),
        WindowMode::Maximized => WindowBounds::Maximized(bounds),
        WindowMode::Fullscreen => WindowBounds::Fullscreen(bounds),
    })
}

/// Shrinks `rect` to fit `area` (but not below `min`, unless `area` is smaller) and
/// moves it fully inside.
fn clamp_into(rect: Rect, area: &Rect, min: (f32, f32)) -> Rect {
    let w = rect.w.max(min.0).min(area.w);
    let h = rect.h.max(min.1).min(area.h);
    Rect::new(
        rect.x.clamp(area.x, area.x + area.w - w),
        rect.y.clamp(area.y, area.y + area.h - h),
        w,
        h,
    )
}

/// What to save for a window: its normal bounds and its mode.
pub fn to_saved(bounds: WindowBounds) -> WindowGeometry {
    let (rect, mode) = match bounds {
        WindowBounds::Windowed(b) => (b, WindowMode::Windowed),
        WindowBounds::Maximized(b) => (b, WindowMode::Maximized),
        WindowBounds::Fullscreen(b) => (b, WindowMode::Fullscreen),
    };
    let rect = Rect::from_gpui(rect);
    WindowGeometry {
        x: rect.x,
        y: rect.y,
        width: rect.w,
        height: rect.h,
        mode,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRIMARY: Rect = Rect::new(0., 0., 1920., 1040.);
    const RIGHT: Rect = Rect::new(1920., 0., 2560., 1400.);
    const MIN: (f32, f32) = (800., 500.);

    fn saved(x: f32, y: f32, w: f32, h: f32, mode: WindowMode) -> WindowGeometry {
        WindowGeometry {
            x,
            y,
            width: w,
            height: h,
            mode,
        }
    }

    fn rect_of(bounds: WindowBounds) -> Rect {
        Rect::from_gpui(bounds.get_bounds())
    }

    #[test]
    fn onboarding_fits_small_screens() {
        // 1366×768 at 125% leaves about 1093×582 logical pixels.
        let laptop = Rect::new(0., 0., 1093., 582.);
        assert_eq!(
            fit_to_display((1088., 792.), (900., 640.), &laptop),
            ((1088., 582.), (900., 582.))
        );
        assert_eq!(
            fit_to_display((1088., 792.), (900., 640.), &PRIMARY),
            ((1088., 792.), (900., 640.))
        );
    }

    #[test]
    fn restores_a_window_that_fits() {
        let s = saved(100., 50., 1200., 800., WindowMode::Windowed);
        let bounds = restore(&s, &[PRIMARY, RIGHT], &PRIMARY, MIN).unwrap();
        assert!(matches!(bounds, WindowBounds::Windowed(_)));
        assert_eq!(rect_of(bounds), Rect::new(100., 50., 1200., 800.));
    }

    #[test]
    fn keeps_mode_and_normal_bounds() {
        let s = saved(2000., 100., 1200., 800., WindowMode::Maximized);
        let bounds = restore(&s, &[PRIMARY, RIGHT], &PRIMARY, MIN).unwrap();
        assert!(matches!(bounds, WindowBounds::Maximized(_)));
        assert_eq!(rect_of(bounds), Rect::new(2000., 100., 1200., 800.));
        let s = WindowGeometry {
            mode: WindowMode::Fullscreen,
            ..s
        };
        let bounds = restore(&s, &[PRIMARY, RIGHT], &PRIMARY, MIN).unwrap();
        assert!(matches!(bounds, WindowBounds::Fullscreen(_)));
    }

    #[test]
    fn unplugged_monitor_falls_back_centered() {
        let s = saved(2200., 100., 1200., 800., WindowMode::Windowed);
        let bounds = restore(&s, &[PRIMARY], &PRIMARY, MIN).unwrap();
        assert_eq!(rect_of(bounds), Rect::new(360., 120., 1200., 800.));
    }

    #[test]
    fn partly_off_screen_is_pulled_in() {
        let s = saved(1500., 900., 1000., 700., WindowMode::Windowed);
        let bounds = restore(&s, &[PRIMARY], &PRIMARY, MIN).unwrap();
        assert_eq!(rect_of(bounds), Rect::new(920., 340., 1000., 700.));
    }

    #[test]
    fn straddling_picks_the_display_with_more_of_it() {
        let s = saved(1700., 100., 1000., 700., WindowMode::Windowed);
        let bounds = restore(&s, &[PRIMARY, RIGHT], &PRIMARY, MIN).unwrap();
        assert_eq!(rect_of(bounds), Rect::new(1920., 100., 1000., 700.));
    }

    #[test]
    fn resolution_drop_shrinks_but_respects_min() {
        let small = Rect::new(0., 0., 1093., 582.);
        let s = saved(0., 0., 1900., 1000., WindowMode::Windowed);
        let bounds = restore(&s, &[small], &small, MIN).unwrap();
        assert_eq!(rect_of(bounds), Rect::new(0., 0., 1093., 582.));
        let s = saved(10., 10., 300., 200., WindowMode::Windowed);
        let bounds = restore(&s, &[PRIMARY], &PRIMARY, MIN).unwrap();
        assert_eq!(rect_of(bounds), Rect::new(10., 10., 800., 500.));
    }

    #[test]
    fn rejects_unsaved_or_garbage() {
        assert!(restore(&WindowGeometry::default(), &[PRIMARY], &PRIMARY, MIN).is_none());
        let nan = saved(f32::NAN, 0., 1000., 700., WindowMode::Windowed);
        assert!(restore(&nan, &[PRIMARY], &PRIMARY, MIN).is_none());
        let inf = saved(0., 0., f32::INFINITY, 700., WindowMode::Windowed);
        assert!(restore(&inf, &[PRIMARY], &PRIMARY, MIN).is_none());
    }

    #[test]
    fn round_trips_through_saved() {
        let bounds = WindowBounds::Maximized(Rect::new(5., 6., 900., 600.).to_gpui());
        let s = to_saved(bounds);
        assert_eq!(s, saved(5., 6., 900., 600., WindowMode::Maximized));
    }
}
