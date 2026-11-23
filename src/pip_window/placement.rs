//! Where the Picture in Picture window goes (S8.5.2), without GPUI: where it was last left on
//! the display it opens on, else in that display's bottom-right corner; always at the video's
//! aspect ratio and inside the display's work area.

use crate::app::geometry::Rect;
use crate::settings::{PipPlace, ScreenRect};
use crate::window_prefs::kept_inside;

/// The window's width the first time it opens on a display.
pub const DEFAULT_WIDTH: f32 = 400.;
/// The narrowest and shortest the window gets.
pub const MIN_SIZE: (f32, f32) = (200., 112.);
/// How far from the work area's edges it first opens.
const MARGIN: f32 = 24.;
/// The most of the work area's width it opens at.
const MAX_SHARE: f32 = 0.6;
/// How many displays' places are remembered.
const MAX_PLACES: usize = 8;
/// How far apart two rectangles' edges may be and still be the same display, in pixels.
const SAME_DISPLAY: f32 = 1.;

/// The window's bounds on a display whose bounds are `display` and work area `area`, for a
/// picture `aspect` wide per unit of height, from `places` remembered.
pub fn place(places: &[PipPlace], display: Rect, area: &Rect, aspect: f32) -> Rect {
    let saved = places.iter().find(|place| same(&from_screen(place.display), &display));
    let width = saved.map_or(DEFAULT_WIDTH, |place| place.window.width);
    let (w, h) = fitted_size(width, aspect, area);
    let origin = saved
        .map_or((area.x + area.w - w - MARGIN, area.y + area.h - h - MARGIN), |place| {
            (place.window.x, place.window.y)
        });
    kept_inside(Rect::new(origin.0, origin.1, w, h), area)
}

/// The window's size `width` wide at `aspect`, kept between [`MIN_SIZE`] and a share of `area`.
pub fn fitted_size(width: f32, aspect: f32, area: &Rect) -> (f32, f32) {
    let aspect = if aspect.is_finite() && aspect > 0. { aspect } else { 16. / 9. };
    let most = (area.w * MAX_SHARE).min(area.h * MAX_SHARE * aspect).max(MIN_SIZE.0);
    let least = MIN_SIZE.0.max(MIN_SIZE.1 * aspect);
    let w = width.clamp(least, most.max(least));
    (w, w / aspect)
}

/// `places` with `window` remembered as the window's place on `display`, newest first.
pub fn remember(places: &mut Vec<PipPlace>, display: Rect, window: Rect) {
    places.retain(|place| !same(&from_screen(place.display), &display));
    places.insert(0, PipPlace { display: to_screen(display), window: to_screen(window) });
    places.truncate(MAX_PLACES);
}

/// Where a window at `window` goes when its middle is on none of `areas` (its display was
/// disconnected): the first area's bottom-right corner. `None` while it is on one.
pub fn rescue(window: Rect, areas: &[Rect]) -> Option<Rect> {
    if areas.iter().any(|area| area.contains(window.center())) {
        return None;
    }
    let area = areas.first()?;
    let origin = (area.x + area.w - window.w - MARGIN, area.y + area.h - window.h - MARGIN);
    Some(kept_inside(Rect::new(origin.0, origin.1, window.w, window.h), area))
}

fn same(a: &Rect, b: &Rect) -> bool {
    (a.x - b.x).abs() < SAME_DISPLAY
        && (a.y - b.y).abs() < SAME_DISPLAY
        && (a.w - b.w).abs() < SAME_DISPLAY
        && (a.h - b.h).abs() < SAME_DISPLAY
}

fn from_screen(rect: ScreenRect) -> Rect {
    Rect::new(rect.x, rect.y, rect.width, rect.height)
}

fn to_screen(rect: Rect) -> ScreenRect {
    ScreenRect { x: rect.x, y: rect.y, width: rect.w, height: rect.h }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAPTOP: Rect = Rect::new(0., 0., 1512., 982.);
    const LAPTOP_AREA: Rect = Rect::new(0., 25., 1512., 957.);
    const MONITOR: Rect = Rect::new(1512., 0., 2560., 1440.);
    const WIDE: f32 = 16. / 9.;

    #[test]
    fn it_first_opens_in_the_bottom_right_corner_at_the_video_aspect() {
        let placed = place(&[], LAPTOP, &LAPTOP_AREA, WIDE);
        assert_eq!((placed.w, placed.h), (400., 225.));
        assert_eq!((placed.x, placed.y), (1512. - 400. - 24., 982. - 225. - 24.));
    }

    #[test]
    fn each_display_keeps_its_own_place() {
        let mut places = Vec::new();
        remember(&mut places, LAPTOP, Rect::new(100., 100., 320., 180.));
        remember(&mut places, MONITOR, Rect::new(2000., 300., 640., 360.));
        let on_laptop = place(&places, LAPTOP, &LAPTOP_AREA, WIDE);
        assert_eq!(on_laptop, Rect::new(100., 100., 320., 180.));
        let on_monitor = place(&places, MONITOR, &MONITOR, WIDE);
        assert_eq!(on_monitor, Rect::new(2000., 300., 640., 360.));
        remember(&mut places, LAPTOP, Rect::new(50., 60., 320., 180.));
        assert_eq!(places.len(), 2, "one place per display");
        assert_eq!(places[0].window.x, 50.);
    }

    #[test]
    fn a_new_aspect_keeps_the_width_and_the_window_stays_on_screen() {
        let mut places = Vec::new();
        remember(&mut places, LAPTOP, Rect::new(1200., 700., 400., 225.));
        let square = place(&places, LAPTOP, &LAPTOP_AREA, 1.);
        assert_eq!((square.w, square.h), (400., 400.));
        assert!(square.y + square.h <= LAPTOP_AREA.y + LAPTOP_AREA.h);
    }

    #[test]
    fn sizes_stay_between_the_minimum_and_a_share_of_the_screen() {
        assert_eq!(fitted_size(50., WIDE, &LAPTOP_AREA).0, 200.);
        assert!(fitted_size(5000., WIDE, &LAPTOP_AREA).0 <= LAPTOP_AREA.w * MAX_SHARE);
        let portrait = fitted_size(400., 9. / 16., &LAPTOP_AREA);
        assert!(portrait.1 <= LAPTOP_AREA.h * MAX_SHARE + 0.01, "{portrait:?}");
        assert_eq!(fitted_size(400., f32::NAN, &LAPTOP_AREA), (400., 225.));
    }

    #[test]
    fn a_window_left_on_a_disconnected_display_comes_back() {
        let lost = Rect::new(3000., 500., 400., 225.);
        let back = rescue(lost, &[LAPTOP_AREA]).expect("moved");
        assert!(LAPTOP_AREA.contains(back.center()));
        assert_eq!(rescue(Rect::new(100., 100., 400., 225.), &[LAPTOP_AREA]), None);
    }

    #[test]
    fn only_a_few_displays_are_remembered() {
        let mut places = Vec::new();
        for i in 0..20 {
            let display = Rect::new(i as f32 * 2000., 0., 1920., 1080.);
            remember(&mut places, display, Rect::new(display.x, 0., 400., 225.));
        }
        assert_eq!(places.len(), MAX_PLACES);
    }
}
