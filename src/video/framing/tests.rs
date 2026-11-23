use gpui::{point, px, size};

use super::*;

fn area() -> Bounds<Pixels> {
    Bounds { origin: point(px(0.), px(0.)), size: size(px(1600.), px(900.)) }
}

fn parts(bounds: Bounds<Pixels>) -> [f32; 4] {
    [bounds.origin.x, bounds.origin.y, bounds.size.width, bounds.size.height].map(f32::from)
}

/// A picture of `pixels`, shown at that shape.
fn source(pixels: (u32, u32)) -> Source {
    Source { aspect: pixels, pixels }
}

fn framed(pixels: (u32, u32), framing: Framing) -> Framed {
    frame(area(), source(pixels), framing, 1.)
}

fn edges(top: u32, bottom: u32, left: u32, right: u32) -> CropEdges {
    CropEdges { enabled: true, top, bottom, left, right }
}

#[test]
fn the_defaults_show_the_whole_picture_at_its_own_shape() {
    let framed = framed((1440, 1080), Framing::default());
    assert_eq!(parts(framed.shown), [200., 0., 1200., 900.]);
    assert_eq!(framed.picture, framed.shown);
}

#[test]
fn an_aspect_ratio_stretches_the_whole_picture() {
    let framed =
        framed((1440, 1080), Framing { aspect_ratio: AspectRatio::Wide, ..Framing::default() });
    assert_eq!(parts(framed.shown), [0., 0., 1600., 900.]);
    assert_eq!(framed.picture, framed.shown);
}

#[test]
fn cropping_a_wide_picture_narrower_trims_its_sides() {
    let framed = framed((1920, 1080), Framing { crop: Crop::Standard, ..Framing::default() });
    assert_eq!(parts(framed.shown), [200., 0., 1200., 900.]);
    assert_eq!(parts(framed.picture), [0., 0., 1600., 900.]);
}

#[test]
fn cropping_to_a_wider_ratio_trims_the_top_and_bottom() {
    let framed = framed((1920, 1080), Framing { crop: Crop::Ratio235, ..Framing::default() });
    let [x, y, width, height] = parts(framed.picture);
    assert_eq!(parts(framed.shown)[2], 1600.);
    assert_eq!((x, width), (0., 1600.));
    // The shown part is snapped to whole pixels first, which scales the picture slightly.
    assert!((height - 900.).abs() < 2., "{height}");
    assert!(y < framed.shown.origin.y.into());
}

#[test]
fn the_crop_applies_after_the_aspect_ratio() {
    let framing =
        Framing { aspect_ratio: AspectRatio::Wide, crop: Crop::Standard, ..Framing::default() };
    let framed = framed((1440, 1080), framing);
    assert_eq!(parts(framed.picture), [0., 0., 1600., 900.]);
}

#[test]
fn trimmed_edges_fall_outside_the_shown_part() {
    // A 4:3 picture inside 16:9 bars: trimming the bars leaves 1440×1080 of 1920×1080.
    let framing = Framing { edges: edges(0, 0, 240, 240), ..Framing::default() };
    let framed = framed((1920, 1080), framing);
    assert_eq!(parts(framed.shown), [200., 0., 1200., 900.]);
    assert_eq!(parts(framed.picture), [0., 0., 1600., 900.]);
}

#[test]
fn uneven_edges_move_the_picture() {
    let framing = Framing { edges: edges(0, 0, 480, 0), ..Framing::default() };
    let framed = framed((1920, 1080), framing);
    let [x, _, width, _] = parts(framed.picture);
    let shown_x = f32::from(framed.shown.origin.x);
    assert!((width - 1600.).abs() < 1., "{width}");
    assert!((shown_x - x - 400.).abs() < 1., "a quarter of the picture is off its left");
}

#[test]
fn fill_covers_the_area_and_clips_the_rest() {
    let framed = framed((1440, 1080), Framing { zoom: Zoom::Fill, ..Framing::default() });
    assert_eq!(parts(framed.shown), [0., 0., 1600., 900.]);
    assert_eq!(parts(framed.picture), [0., -150., 1600., 1200.]);
}

#[test]
fn fixed_zooms_count_video_pixels_as_device_pixels() {
    let half = Framing { zoom: Zoom::Half, ..Framing::default() };
    let framed = frame(area(), source((1920, 1080)), half, 2.);
    assert_eq!(parts(framed.shown), [560., 315., 480., 270.]);
    let original = Framing { zoom: Zoom::Original, ..Framing::default() };
    let framed = frame(area(), source((1920, 1080)), original, 1.);
    assert_eq!(parts(framed.shown), [0., 0., 1600., 900.], "clipped to the area");
    assert_eq!(parts(framed.picture), [-160., -90., 1920., 1080.]);
}

#[test]
fn a_custom_ratio_stretches_the_whole_picture() {
    let custom = Framing {
        aspect_ratio: AspectRatio::Custom,
        custom_ratio: parse_ratio("16:9"),
        ..Framing::default()
    };
    assert_eq!(parts(framed((1440, 1080), custom).shown), [0., 0., 1600., 900.]);
    let unreadable = Framing { custom_ratio: None, ..custom };
    assert_eq!(parts(framed((1440, 1080), unreadable).shown), [200., 0., 1200., 900.]);
}
