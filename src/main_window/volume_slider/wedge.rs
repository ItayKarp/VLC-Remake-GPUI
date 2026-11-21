//! The wedge (S2.5.1, `volume/volume-*.png`): narrow and rounded on the left, tall on the right,
//! a dark track with the fill inset 1.5 px inside it. The fill is painted in straight gradient
//! stretches ([`Segment`]) whose colours belong to the levels, so a louder volume reveals more
//! of the same gradient instead of stretching it.

use gpui::{
    Bounds, Hsla, PathBuilder, Pixels, Window, linear_color_stop, linear_gradient, point, px,
};

use super::scale::Segment;

/// The wedge's width, measured from the designs.
pub const WEDGE_WIDTH: f32 = 89.;
/// The wedge's height at its tall right end.
pub const WEDGE_HEIGHT: f32 = 27.5;
/// The wedge's height at its narrow left end.
const NARROW: f32 = 10.5;
/// The radius of the track's left corners, which round its narrow end.
const LEFT_CORNER: f32 = 3.;
/// How far the fill sits inside the track.
const INSET: f32 = 1.5;
/// The radius of the track's right corners.
const CORNER: f32 = 2.;
/// How far apart the outline's points are, in pixels; fine enough for smooth round ends.
const SAMPLE: f32 = 0.5;
/// Gradient angle that runs left to right.
const LEFT_TO_RIGHT: f32 = 90.;

/// A wedge outline whose height grows linearly from the left end to the right end, with
/// rounded corners.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Profile {
    width: f32,
    narrow: f32,
    tall: f32,
    left_corner: f32,
    corner: f32,
}

/// The track's outline.
const TRACK: Profile = Profile {
    width: WEDGE_WIDTH,
    narrow: NARROW,
    tall: WEDGE_HEIGHT,
    left_corner: LEFT_CORNER,
    corner: CORNER,
};
/// The fill's outline, inset in the track.
const FILL: Profile = Profile {
    width: WEDGE_WIDTH - 2. * INSET,
    narrow: NARROW - 2. * INSET,
    tall: WEDGE_HEIGHT - 2. * INSET,
    left_corner: LEFT_CORNER - INSET,
    corner: CORNER - INSET / 2.,
};

impl Profile {
    /// Half the outline's height `x` pixels from its left edge.
    fn half_height(self, x: f32) -> f32 {
        let x = x.clamp(0., self.width);
        let low = self.narrow / 2.;
        let straight = low + (self.tall / 2. - low) * x / self.width;
        // The corners take a little off the straight edges near both ends.
        let corner = |distance: f32, radius: f32| {
            round_end(distance, radius).map_or(straight, |half| straight - radius + half)
        };
        corner(x, self.left_corner).min(corner(self.width - x, self.corner))
    }

    /// Points along the outline between `from` and `to`, as (x, half height).
    fn samples(self, from: f32, to: f32) -> Vec<(f32, f32)> {
        let count = ((to - from) / SAMPLE).ceil().max(1.) as usize;
        (0..=count)
            .map(|i| from + (to - from) * i as f32 / count as f32)
            .map(|x| (x, self.half_height(x)))
            .collect()
    }
}

/// Half the height of a circle of `radius` at `distance` from its outer edge, when that is
/// still inside its curved part.
fn round_end(distance: f32, radius: f32) -> Option<f32> {
    (distance < radius).then(|| (radius * radius - (radius - distance).powi(2)).max(0.).sqrt())
}

/// What the wedge shows.
pub struct Look {
    /// The track's colour.
    pub track: Hsla,
    /// The fill, in gradient stretches along the track.
    pub segments: Vec<Segment>,
    /// How opaque the fill is: dimmed while muted or unavailable.
    pub opacity: f32,
}

/// Paints the wedge into `bounds`, vertically centred.
pub fn paint(bounds: Bounds<Pixels>, look: &Look, window: &mut Window) {
    let left = f32::from(bounds.left());
    let middle = f32::from(bounds.center().y);
    paint_part(&TRACK.samples(0., TRACK.width), (left, middle), look.track.into(), window);
    let fill_left = left + INSET;
    for segment in &look.segments {
        let (from, to) = (segment.from * FILL.width, segment.to * FILL.width);
        let gradient = linear_gradient(
            LEFT_TO_RIGHT,
            linear_color_stop(segment.start.opacity(look.opacity), 0.),
            linear_color_stop(segment.end.opacity(look.opacity), 1.),
        );
        paint_part(&FILL.samples(from, to), (fill_left, middle), gradient, window);
    }
}

/// Fills the outline through `samples`, with x measured from `left` and heights centred on
/// `middle`.
fn paint_part(
    samples: &[(f32, f32)],
    (left, middle): (f32, f32),
    background: gpui::Background,
    window: &mut Window,
) {
    let Some(&(first_x, first_half)) = samples.first() else { return };
    let at = |x: f32, y: f32| point(px(left + x), px(y));
    let mut path = PathBuilder::fill();
    path.move_to(at(first_x, middle - first_half));
    for &(x, half) in &samples[1..] {
        path.line_to(at(x, middle - half));
    }
    for &(x, half) in samples.iter().rev() {
        path.line_to(at(x, middle + half));
    }
    path.close();
    match path.build() {
        Ok(path) => window.paint_path(path, background),
        Err(error) => tracing::warn!("volume: couldn't build the wedge: {error:#}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wedge_grows_from_its_rounded_left_end() {
        let rounded = NARROW / 2. - LEFT_CORNER;
        assert!((TRACK.half_height(0.) - rounded).abs() < 0.01, "the corners round the end");
        assert!((TRACK.half_height(LEFT_CORNER) - NARROW / 2.).abs() < 0.5);
        let middle = TRACK.half_height(WEDGE_WIDTH / 2.);
        assert!(middle > NARROW / 2. && middle < WEDGE_HEIGHT / 2.);
        assert!((TRACK.half_height(WEDGE_WIDTH - CORNER) - WEDGE_HEIGHT / 2.).abs() < 0.5);
    }

    #[test]
    fn the_right_corners_are_rounded() {
        assert!(TRACK.half_height(WEDGE_WIDTH) < WEDGE_HEIGHT / 2. - CORNER + 0.01);
    }

    #[test]
    fn samples_cover_the_whole_stretch() {
        let samples = FILL.samples(10., 20.);
        assert_eq!(samples.first().map(|s| s.0), Some(10.));
        assert_eq!(samples.last().map(|s| s.0), Some(20.));
        assert_eq!(FILL.samples(5., 5.).len(), 2);
    }
}
