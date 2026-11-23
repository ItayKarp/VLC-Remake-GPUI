//! How the picture is framed in the video area (S5.8.1, S7.2): the aspect ratio it is stretched
//! to, the edges and the shape it is cropped to, and how large it is drawn.
//!
//! libVLC can't do any of it here: it draws into our own frames (`vmem`), and its aspect ratio,
//! crop and zoom only change how its own windows show the picture. So the video view does: it
//! works out the part of the picture that shows and where it goes, then draws the whole picture
//! larger, clipped to that part, so the trimmed edges fall outside.
//!
//! In order: the aspect ratio stretches the whole picture; the edges trim it in video pixels;
//! the crop's shape keeps the middle of what is left; the zoom sizes that part, which is
//! clipped to the video area when larger.

mod edges;
mod ratios;
mod zoom;

use gpui::{Bounds, Pixels, point, px, size};

pub use edges::{CropEdges, MIN_KEPT};
pub use ratios::{AspectRatio, Crop, RATIO_HELP, parse_ratio};
pub use zoom::Zoom;

use super::layout;
use crate::settings::VideoSettings;

/// How a picture is framed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Framing {
    /// The shape the whole picture is stretched to.
    pub aspect_ratio: AspectRatio,
    /// That shape when it is Custom, as width and height.
    pub custom_ratio: Option<(u32, u32)>,
    /// The video pixels trimmed off its sides.
    pub edges: CropEdges,
    /// The shape the rest is cropped to, keeping its middle.
    pub crop: Crop,
    /// How large the part that shows is drawn.
    pub zoom: Zoom,
}

impl Framing {
    /// The framing `video` settings ask for.
    pub fn of(video: &VideoSettings) -> Self {
        Self {
            aspect_ratio: video.aspect_ratio,
            custom_ratio: parse_ratio(&video.custom_aspect_ratio),
            edges: video.crop_edges,
            crop: video.crop,
            zoom: video.zoom,
        }
    }
}

/// The picture being framed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Source {
    /// Its display shape, width by height.
    pub aspect: (u32, u32),
    /// Its size in video pixels, which the edges and the fixed zooms count.
    pub pixels: (u32, u32),
}

/// Where a framed picture goes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Framed {
    /// The part that shows, inside the video area; drawing is clipped to it.
    pub shown: Bounds<Pixels>,
    /// Where the whole picture is drawn: `shown`, or larger when trimmed or clipped.
    pub picture: Bounds<Pixels>,
}

/// Places `source` in `container` as `framing` says, on the device pixels of a screen with
/// `scale` pixels per point.
pub fn frame(container: Bounds<Pixels>, source: Source, framing: Framing, scale: f32) -> Framed {
    let whole = match framing.aspect_ratio {
        AspectRatio::Custom => framing.custom_ratio,
        other => other.ratio(""),
    };
    let whole = ratio(whole.unwrap_or(source.aspect));
    let trim = framing.edges.trim(source.pixels);
    let trimmed = whole * trim.kept_width() / trim.kept_height().max(f32::MIN_POSITIVE);
    let (cropped, (crop_width, crop_height)) = framing
        .crop
        .ratio()
        .map_or((trimmed, (1., 1.)), |crop| (ratio(crop), kept(trimmed, ratio(crop))));
    // The part that shows, as shares of the whole picture.
    let (kept_width, kept_height) =
        (trim.kept_width() * crop_width, trim.kept_height() * crop_height);
    let left = trim.left + trim.kept_width() * (1. - crop_width) / 2.;
    let top = trim.top + trim.kept_height() * (1. - crop_height) / 2.;
    let shown = sized(container, cropped, shown_height(source, framing.zoom, kept_height), scale);
    let (width, height) = (f32::from(shown.size.width), f32::from(shown.size.height));
    let (full_width, full_height) = (width / kept_width, height / kept_height);
    let picture = Bounds {
        origin: point(
            shown.origin.x - px(left * full_width),
            shown.origin.y - px(top * full_height),
        ),
        size: size(px(full_width), px(full_height)),
    };
    Framed { shown: shown.intersect(&container), picture }
}

/// How the shown part is sized: to fit or fill the area, or at a height in points.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Height {
    Fit,
    Fill,
    Points(f32),
}

/// How tall `zoom` draws the shown part, `kept_height` of the picture's height.
fn shown_height(source: Source, zoom: Zoom, kept_height: f32) -> Height {
    match (zoom, zoom.factor()) {
        (Zoom::Fill, _) => Height::Fill,
        (_, Some(factor)) => Height::Points(source.pixels.1 as f32 * kept_height * factor),
        (_, None) => Height::Fit,
    }
}

/// The rectangle of shape `ratio` centered in `container` and sized as `height` says, its edges
/// on device pixels.
fn sized(container: Bounds<Pixels>, ratio: f32, height: Height, scale: f32) -> Bounds<Pixels> {
    let (width, tall) = (f32::from(container.size.width), f32::from(container.size.height));
    let wider = width / tall.max(f32::MIN_POSITIVE) > ratio;
    let height = match height {
        Height::Fit if wider => tall,
        Height::Fit => width / ratio,
        Height::Fill if wider => width / ratio,
        Height::Fill => tall,
        // Video pixels are device pixels at 1:1, so a point holds `scale` of them.
        Height::Points(pixels) => pixels / scale,
    };
    layout::centered(container, (height * ratio, height), scale)
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
mod tests;
