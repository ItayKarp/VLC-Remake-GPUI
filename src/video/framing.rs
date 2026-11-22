//! Settings → Video's Framing (S5.8.1): the aspect ratio the picture is shown at, and the crop
//! that trims it to another ratio.
//!
//! libVLC can't do either here: it draws into our own frames (`vmem`), and its aspect ratio and
//! crop only change how its own windows show the picture. So the video view does both: it
//! letterboxes the picture at the chosen ratio, then draws it larger than the shown part, clipped
//! to it, so the trimmed edges fall outside.

use gpui::{Bounds, Pixels, point, px, size};
use serde::{Deserialize, Serialize};

use super::layout;

/// "Aspect ratio": the shape the whole picture is stretched to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AspectRatio {
    /// The media's own.
    #[default]
    Automatic,
    /// 1:1.
    Square,
    /// 4:3.
    Standard,
    /// 5:4.
    FiveByFour,
    /// 16:9.
    Wide,
    /// 16:10.
    SixteenByTen,
    /// 2.21:1 (70 mm).
    Ratio221,
    /// 2.35:1 (CinemaScope).
    Ratio235,
    /// 2.39:1 (anamorphic widescreen).
    Ratio239,
}

impl AspectRatio {
    /// The ratio as width and height; `None` keeps the media's own.
    pub fn ratio(self) -> Option<(u32, u32)> {
        match self {
            Self::Automatic => None,
            Self::Square => Some((1, 1)),
            Self::Standard => Some((4, 3)),
            Self::FiveByFour => Some((5, 4)),
            Self::Wide => Some((16, 9)),
            Self::SixteenByTen => Some((16, 10)),
            Self::Ratio221 => Some((221, 100)),
            Self::Ratio235 => Some((235, 100)),
            Self::Ratio239 => Some((239, 100)),
        }
    }
}

/// "Crop": the ratio the picture is trimmed to, keeping its middle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Crop {
    /// The whole picture.
    #[default]
    None,
    /// 1:1.
    Square,
    /// 4:3.
    Standard,
    /// 16:9.
    Wide,
    /// 16:10.
    SixteenByTen,
    /// 1.85:1.
    Ratio185,
    /// 2.21:1.
    Ratio221,
    /// 2.35:1.
    Ratio235,
    /// 2.39:1.
    Ratio239,
}

impl Crop {
    /// The ratio as width and height; `None` keeps the whole picture.
    pub fn ratio(self) -> Option<(u32, u32)> {
        match self {
            Self::None => None,
            Self::Square => Some((1, 1)),
            Self::Standard => Some((4, 3)),
            Self::Wide => Some((16, 9)),
            Self::SixteenByTen => Some((16, 10)),
            Self::Ratio185 => Some((185, 100)),
            Self::Ratio221 => Some((221, 100)),
            Self::Ratio235 => Some((235, 100)),
            Self::Ratio239 => Some((239, 100)),
        }
    }
}

/// Where a framed picture goes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Framed {
    /// The part that shows, letterboxed in the video area; drawing is clipped to it.
    pub shown: Bounds<Pixels>,
    /// Where the whole picture is drawn: `shown`, or larger when cropped.
    pub picture: Bounds<Pixels>,
}

/// Places a picture of display `aspect` in `container`, stretched to `aspect_ratio` and
/// trimmed to `crop`, on the device pixels of a screen with `scale` pixels per point.
pub fn frame(
    container: Bounds<Pixels>,
    aspect: (u32, u32),
    aspect_ratio: AspectRatio,
    crop: Crop,
    scale: f32,
) -> Framed {
    let aspect = aspect_ratio.ratio().unwrap_or(aspect);
    let Some(cropped) = crop.ratio() else {
        let shown = layout::letterbox(container, aspect, scale);
        return Framed { shown, picture: shown };
    };
    let shown = layout::letterbox(container, cropped, scale);
    let (kept_width, kept_height) = kept(ratio(aspect), ratio(cropped));
    let (width, height) = (f32::from(shown.size.width), f32::from(shown.size.height));
    let (full_width, full_height) = (width / kept_width, height / kept_height);
    let center = shown.center();
    let picture = Bounds {
        origin: point(center.x - px(full_width / 2.), center.y - px(full_height / 2.)),
        size: size(px(full_width), px(full_height)),
    };
    Framed { shown, picture }
}

/// The shares of the picture's width and height a crop to `cropped` keeps from a picture of
/// ratio `picture`: the whole height of a picture wider than the crop, else its whole width.
fn kept(picture: f32, cropped: f32) -> (f32, f32) {
    if picture > cropped { (cropped / picture, 1.) } else { (1., picture / cropped) }
}

/// Width over height.
fn ratio((width, height): (u32, u32)) -> f32 {
    width.max(1) as f32 / height.max(1) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> Bounds<Pixels> {
        Bounds { origin: point(px(0.), px(0.)), size: size(px(1600.), px(900.)) }
    }

    fn parts(bounds: Bounds<Pixels>) -> [f32; 4] {
        [bounds.origin.x, bounds.origin.y, bounds.size.width, bounds.size.height].map(f32::from)
    }

    #[test]
    fn automatic_and_no_crop_show_the_whole_picture_at_its_own_shape() {
        let framed = frame(area(), (1440, 1080), AspectRatio::Automatic, Crop::None, 1.);
        assert_eq!(parts(framed.shown), [200., 0., 1200., 900.]);
        assert_eq!(framed.picture, framed.shown);
    }

    #[test]
    fn an_aspect_ratio_stretches_the_whole_picture() {
        let framed = frame(area(), (1440, 1080), AspectRatio::Wide, Crop::None, 1.);
        assert_eq!(parts(framed.shown), [0., 0., 1600., 900.]);
        assert_eq!(framed.picture, framed.shown);
    }

    #[test]
    fn cropping_a_wide_picture_narrower_trims_its_sides() {
        let framed = frame(area(), (1920, 1080), AspectRatio::Automatic, Crop::Standard, 1.);
        assert_eq!(parts(framed.shown), [200., 0., 1200., 900.]);
        assert_eq!(parts(framed.picture), [0., 0., 1600., 900.]);
    }

    #[test]
    fn cropping_to_a_wider_ratio_trims_the_top_and_bottom() {
        let framed = frame(area(), (1920, 1080), AspectRatio::Automatic, Crop::Ratio235, 1.);
        let [x, y, width, height] = parts(framed.picture);
        assert_eq!(parts(framed.shown)[2], 1600.);
        assert_eq!((x, width), (0., 1600.));
        // The shown part is snapped to whole pixels first, which scales the picture slightly.
        assert!((height - 900.).abs() < 2., "{height}");
        assert!(y < framed.shown.origin.y.into());
    }

    #[test]
    fn the_crop_applies_after_the_aspect_ratio() {
        let framed = frame(area(), (1440, 1080), AspectRatio::Wide, Crop::Standard, 1.);
        assert_eq!(parts(framed.picture), [0., 0., 1600., 900.]);
    }
}
