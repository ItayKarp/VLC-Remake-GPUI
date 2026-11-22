//! "Resize window to fit video" (S5.4.1) without GPUI: the window size that shows a video at its
//! own size, shrunk to the screen's work area, and where the window must go to stay on it.

use crate::app::geometry::Rect;

/// The window size showing a `video`-sized picture with `chrome` around it (the bars and the
/// viewport's inset), kept within `area` and at least `min` (or the whole area, if smaller).
/// Portrait and 8K video shrink to the area with their aspect kept.
pub fn window_size(
    video: (f32, f32),
    chrome: (f32, f32),
    area: &Rect,
    min: (f32, f32),
) -> (f32, f32) {
    let room = ((area.w - chrome.0).max(1.), (area.h - chrome.1).max(1.));
    let scale = (room.0 / video.0).min(room.1 / video.1).min(1.);
    let (w, h) = (video.0 * scale + chrome.0, video.1 * scale + chrome.1);
    (w.max(min.0).min(area.w), h.max(min.1).min(area.h))
}

/// `window` moved as little as needed to lie inside `area`.
pub fn kept_inside(window: Rect, area: &Rect) -> Rect {
    let x = window.x.clamp(area.x, (area.x + area.w - window.w).max(area.x));
    let y = window.y.clamp(area.y, (area.y + area.h - window.h).max(area.y));
    Rect::new(x, y, window.w, window.h)
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: Rect = Rect::new(0., 25., 1512., 957.);
    const MIN: (f32, f32) = (800., 500.);
    const CHROME: (f32, f32) = (20., 90.);

    #[test]
    fn a_video_that_fits_shows_at_its_own_size() {
        assert_eq!(window_size((1280., 720.), CHROME, &AREA, MIN), (1300., 810.));
    }

    #[test]
    fn big_and_portrait_videos_shrink_to_the_area_keeping_their_aspect() {
        let (w, h) = window_size((7680., 4320.), CHROME, &AREA, MIN);
        assert_eq!(w, 1512.);
        assert!(((w - 20.) / (h - 90.) - 16. / 9.).abs() < 0.01);
        let (w, h) = window_size((1080., 1920.), CHROME, &AREA, MIN);
        assert_eq!(h, 957.);
        assert!(w < h);
        assert!(w >= MIN.0);
    }

    #[test]
    fn small_videos_get_the_minimum_size() {
        assert_eq!(window_size((320., 240.), CHROME, &AREA, MIN), (800., 500.));
        let tiny = Rect::new(0., 0., 640., 400.);
        assert_eq!(window_size((320., 240.), CHROME, &tiny, MIN), (640., 400.));
    }

    #[test]
    fn a_window_grown_past_the_edge_moves_back_in() {
        let grown = Rect::new(900., 400., 1300., 810.);
        assert_eq!(kept_inside(grown, &AREA), Rect::new(212., 172., 1300., 810.));
        let inside = Rect::new(10., 30., 800., 500.);
        assert_eq!(kept_inside(inside, &AREA), inside);
    }
}
