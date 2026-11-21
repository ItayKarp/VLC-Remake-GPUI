//! Where the picture goes inside the video area (S1.4.1): the largest rectangle of the video's
//! display aspect that fits, centred, with its edges on whole device pixels so HiDPI screens
//! show no blurred or half-covered row at the bars.

use gpui::{Bounds, Pixels, point, px, size};

/// The picture's rectangle inside `container` for a video shown at `aspect` (width, height,
/// in square pixels), snapped to the device pixels of a screen with `scale` pixels per point.
pub fn letterbox(container: Bounds<Pixels>, aspect: (u32, u32), scale: f32) -> Bounds<Pixels> {
    let (left, top) = (f32::from(container.origin.x), f32::from(container.origin.y));
    let (width, height) = (f32::from(container.size.width), f32::from(container.size.height));
    let ratio = aspect.0.max(1) as f32 / aspect.1.max(1) as f32;
    let (fit_width, fit_height) = if width / height.max(f32::MIN_POSITIVE) > ratio {
        (height * ratio, height)
    } else {
        (width, width / ratio)
    };
    let snap = |value: f32| (value * scale).round() / scale;
    let (center_x, center_y) = (left + width / 2.0, top + height / 2.0);
    let (x0, x1) = (snap(center_x - fit_width / 2.0), snap(center_x + fit_width / 2.0));
    let (y0, y1) = (snap(center_y - fit_height / 2.0), snap(center_y + fit_height / 2.0));
    Bounds { origin: point(px(x0), px(y0)), size: size(px(x1 - x0), px(y1 - y0)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(width: f32, height: f32) -> Bounds<Pixels> {
        Bounds { origin: point(px(0.0), px(40.0)), size: size(px(width), px(height)) }
    }

    fn parts(bounds: Bounds<Pixels>) -> [f32; 4] {
        [bounds.origin.x, bounds.origin.y, bounds.size.width, bounds.size.height].map(f32::from)
    }

    #[test]
    fn wide_video_gets_bars_above_and_below() {
        let placed = letterbox(area(1280.0, 720.0), (1920, 800), 1.0);
        assert_eq!(parts(placed), [0.0, 133.0, 1280.0, 534.0]);
    }

    #[test]
    fn tall_video_gets_bars_at_the_sides() {
        let placed = letterbox(area(1280.0, 720.0), (1080, 1920), 1.0);
        assert_eq!(parts(placed), [438.0, 40.0, 405.0, 720.0]);
    }

    #[test]
    fn matching_aspect_fills_the_area() {
        let placed = letterbox(area(1280.0, 720.0), (1920, 1080), 2.0);
        assert_eq!(parts(placed), [0.0, 40.0, 1280.0, 720.0]);
    }

    #[test]
    fn edges_land_on_device_pixels() {
        let placed = letterbox(area(1001.0, 700.0), (853, 480), 2.0);
        for value in parts(placed) {
            assert_eq!((value * 2.0).fract(), 0.0, "{value} is not on a device pixel");
        }
        let fractional = letterbox(area(1001.0, 700.0), (853, 480), 1.5);
        for value in parts(fractional) {
            assert!(((value * 1.5) - (value * 1.5).round()).abs() < 1e-3, "{value} off grid");
        }
    }

    #[test]
    fn an_empty_area_places_nothing_visible() {
        let placed = letterbox(area(0.0, 0.0), (1920, 1080), 1.0);
        assert_eq!(parts(placed)[2..], [0.0, 0.0]);
    }
}
