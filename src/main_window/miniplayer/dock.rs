//! Where the miniplayer card goes (S4.10.3, S4.10.5), decided without GPUI: its size for the
//! room the library leaves it, the corner it docks in, and the corner a drag drops it in.

use crate::settings::MiniplayerCorner;

/// The card's width in the designs (`player_03`), the narrowest it shrinks to before it hides,
/// and the share of the library's width it may take.
pub const CARD_WIDTH: f32 = 390.;
const MIN_CARD_WIDTH: f32 = 240.;
const MAX_SHARE: f32 = 0.5;
/// The card's padding round the video, the progress bar under the video and the gap above it,
/// and the name's and chips' lines.
pub const PAD: f32 = 8.;
pub const PROGRESS_GAP: f32 = 6.;
pub const PROGRESS_HEIGHT: f32 = 14.;
pub const NAME_HEIGHT: f32 = 40.;
pub const CHIPS_HEIGHT: f32 = 40.;
/// How far the card sits from the library's edges.
pub const MARGIN: f32 = 8.;

/// A rectangle in window coordinates: where the card may go.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    /// The left edge.
    pub left: f32,
    /// The top edge.
    pub top: f32,
    /// The width.
    pub width: f32,
    /// The height.
    pub height: f32,
}

/// The card's size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardSize {
    /// The card's width.
    pub width: f32,
    /// The video's height inside it (16:9 of its width inside the padding).
    pub video_height: f32,
}

impl CardSize {
    /// The card's size in `area`: as designed, narrower in a narrow library, and none when it
    /// would be too small to use (S4.10 edge case).
    pub fn fit(area: Area) -> Option<Self> {
        let width = CARD_WIDTH.min(area.width * MAX_SHARE);
        let size = Self { width, video_height: (width - 2. * PAD) * 9. / 16. };
        let fits = width >= MIN_CARD_WIDTH && size.height() + 2. * MARGIN <= area.height;
        fits.then_some(size)
    }

    /// The card's height.
    pub fn height(self) -> f32 {
        PAD + self.video_height + PROGRESS_GAP + PROGRESS_HEIGHT + NAME_HEIGHT + CHIPS_HEIGHT
    }
}

/// Where a card `size` wide and `height` tall sits in `area` when docked in `corner`.
pub fn origin(corner: MiniplayerCorner, area: Area, (width, height): (f32, f32)) -> (f32, f32) {
    let left = area.left + MARGIN;
    let right = area.left + area.width - MARGIN - width;
    let top = area.top + MARGIN;
    let bottom = area.top + area.height - MARGIN - height;
    match corner {
        MiniplayerCorner::TopLeft => (left, top),
        MiniplayerCorner::TopRight => (right, top),
        MiniplayerCorner::BottomLeft => (left, bottom),
        MiniplayerCorner::BottomRight => (right, bottom),
    }
}

/// The corner of `area` nearest a card centered at `(x, y)`: the one in its quarter.
pub fn nearest(area: Area, (x, y): (f32, f32)) -> MiniplayerCorner {
    let left = x < area.left + area.width / 2.;
    let top = y < area.top + area.height / 2.;
    match (top, left) {
        (true, true) => MiniplayerCorner::TopLeft,
        (true, false) => MiniplayerCorner::TopRight,
        (false, true) => MiniplayerCorner::BottomLeft,
        (false, false) => MiniplayerCorner::BottomRight,
    }
}

/// Where a dragged card goes: `(x, y)`, kept inside `area` with its margin.
pub fn clamp(area: Area, (width, height): (f32, f32), (x, y): (f32, f32)) -> (f32, f32) {
    let x = x.min(area.left + area.width - MARGIN - width).max(area.left + MARGIN);
    let y = y.min(area.top + area.height - MARGIN - height).max(area.top + MARGIN);
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIBRARY: Area = Area { left: 217., top: 40., width: 1703., height: 1040. };

    #[test]
    fn a_wide_library_has_the_designed_card() {
        let size = CardSize::fit(LIBRARY).expect("room for the card");
        assert!((size.width - CARD_WIDTH).abs() < f32::EPSILON);
        assert!((size.video_height - 210.375).abs() < 0.01);
    }

    #[test]
    fn a_narrow_library_shrinks_then_hides_the_card() {
        let narrow = Area { width: 600., ..LIBRARY };
        assert!((CardSize::fit(narrow).expect("still fits").width - 300.).abs() < f32::EPSILON);
        assert_eq!(CardSize::fit(Area { width: 400., ..LIBRARY }), None);
        assert_eq!(CardSize::fit(Area { height: 200., ..LIBRARY }), None);
    }

    #[test]
    fn the_card_docks_in_each_corner_inside_the_margin() {
        let size = (390., 330.);
        assert_eq!(origin(MiniplayerCorner::TopLeft, LIBRARY, size), (225., 48.));
        assert_eq!(origin(MiniplayerCorner::BottomRight, LIBRARY, size), (1522., 742.));
    }

    #[test]
    fn a_drop_snaps_to_the_quarter_it_lands_in() {
        assert_eq!(nearest(LIBRARY, (300., 100.)), MiniplayerCorner::TopLeft);
        assert_eq!(nearest(LIBRARY, (1800., 100.)), MiniplayerCorner::TopRight);
        assert_eq!(nearest(LIBRARY, (300., 1000.)), MiniplayerCorner::BottomLeft);
        assert_eq!(nearest(LIBRARY, (1800., 1000.)), MiniplayerCorner::BottomRight);
    }

    #[test]
    fn a_dragged_card_stays_in_the_library() {
        let size = (390., 330.);
        assert_eq!(clamp(LIBRARY, size, (0., 0.)), (225., 48.));
        assert_eq!(clamp(LIBRARY, size, (5000., 5000.)), (1522., 742.));
        assert_eq!(clamp(LIBRARY, size, (700., 300.)), (700., 300.));
    }
}
