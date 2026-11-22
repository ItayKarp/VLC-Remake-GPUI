//! Subtitles the app draws over the video (S6.2): the cues showing now, in Settings →
//! Subtitles' Appearance, laid out on the picture or in the black bars, scaled with the picture,
//! and kept clear of the control bar and title bar while they float over the video.
//!
//! Sizes follow libVLC's text renderer, so a file looks the same drawn here or there: a relative
//! size is the picture's height over VLC's divisor (Automatic is VLC's 6.25%), and a pixel size
//! is in the video's own pixels, scaled as the picture is. Lines wrap inside 90% of the
//! picture's width, and cues showing together stack instead of overlapping.
//!
//! The shaping and painting are in `paint.rs`; this module decides where things go.

mod paint;

use gpui::{Bounds, Pixels, px};

use crate::engine::cues::{Cue, Placement};
use crate::engine::subtitle_style::{RelativeSize, SubtitleStyle, TextSize};
use crate::settings::SubtitleArea;

pub use paint::paint;

/// The divisor Automatic uses: VLC's default text is 6.25% of the picture's height.
const AUTOMATIC_DIVISOR: f32 = 16.;
/// The smallest text drawn, so a tiny window still shows something readable.
const MIN_FONT_SIZE: f32 = 8.;
/// How far text stays from the picture's top and bottom edges, per picture height.
const MARGIN: f32 = 0.04;
/// How much of the picture's width lines may take before they wrap.
const SAFE_WIDTH: f32 = 0.9;
/// The gap kept between subtitles and a bar over the video.
const BAR_GAP: f32 = 8.;

/// What to draw: the cues showing now, and how.
#[derive(Clone, Debug)]
pub struct Overlay {
    /// The cues showing, earliest first.
    pub cues: Vec<Cue>,
    /// The user's style.
    pub style: SubtitleStyle,
    /// Embedded Sub Format: colors the file gives its text are drawn.
    pub file_colors: bool,
    /// Where the text sits.
    pub area: SubtitleArea,
}

/// How much of the video's top and bottom the bars floating over it cover.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Covered {
    /// Under the title bar.
    pub top: Pixels,
    /// Under the control bar.
    pub bottom: Pixels,
}

/// Where the overlay is drawn.
#[derive(Clone, Copy, Debug)]
pub struct Placing {
    /// The whole video area, black bars included.
    pub view: Bounds<Pixels>,
    /// The picture inside it.
    pub picture: Bounds<Pixels>,
    /// The picture's height in the video's own pixels, for pixel sizes.
    pub video_height: u32,
    /// What the bars over the view cover.
    pub covered: Covered,
}

/// The band text is laid out in: centered on `center`, `width` wide, between `top` and
/// `bottom`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Band {
    center: Pixels,
    width: Pixels,
    top: Pixels,
    bottom: Pixels,
}

impl Band {
    /// The band for `area` in `placing`, kept clear of the bars.
    fn new(area: SubtitleArea, placing: &Placing) -> Self {
        let (view, picture) = (placing.view, placing.picture);
        let margin = picture.size.height * MARGIN;
        let outer = match area {
            SubtitleArea::Picture => picture,
            SubtitleArea::Bars => view,
        };
        let covered = placing.covered;
        let mut bottom = outer.bottom() - margin;
        if covered.bottom > px(0.) {
            bottom = bottom.min(view.bottom() - covered.bottom - px(BAR_GAP));
        }
        let mut top = outer.top() + margin;
        if covered.top > px(0.) {
            top = top.max(view.top() + covered.top + px(BAR_GAP));
        }
        Self { center: picture.center().x, width: picture.size.width * SAFE_WIDTH, top, bottom }
    }
}

/// The text's size for `size` on a picture `picture_height` high showing a video
/// `video_height` pixels high.
fn font_size(size: TextSize, picture_height: Pixels, video_height: u32) -> Pixels {
    let height = f32::from(picture_height);
    let points = match size {
        TextSize::Relative(relative) => height / divisor(relative),
        TextSize::Pixels(pixels) if video_height > 0 => {
            pixels as f32 * height / video_height as f32
        }
        TextSize::Pixels(pixels) => pixels as f32,
    };
    px(points.max(MIN_FONT_SIZE))
}

/// The picture's height over the text's for `size`.
fn divisor(size: RelativeSize) -> f32 {
    match size.libvlc_value() {
        0 => AUTOMATIC_DIVISOR,
        divisor => divisor as f32,
    }
}

/// Where each of `heights` (in cue order, all at `placement`) starts in `band`: the bottom
/// stack grows upwards with the earliest cue lowest, the top stack downwards, the middle one
/// around the band's middle.
fn stack(placement: Placement, heights: &[Pixels], band: &Band) -> Vec<Pixels> {
    let total: Pixels = heights.iter().copied().sum();
    let mut y = match placement {
        Placement::Bottom => band.bottom,
        Placement::Top => band.top,
        Placement::Middle => (band.top + band.bottom - total) / 2.,
    };
    heights
        .iter()
        .map(|height| {
            if placement == Placement::Bottom {
                y -= *height;
                y
            } else {
                let at = y;
                y += *height;
                at
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use gpui::{point, size};

    use super::*;

    fn placing(bottom: f32) -> Placing {
        // A 16:9 picture letterboxed in a 1000×800 view.
        let view = Bounds::new(point(px(0.), px(0.)), size(px(1000.), px(800.)));
        let picture = Bounds::new(point(px(0.), px(118.75)), size(px(1000.), px(562.5)));
        let covered = Covered { top: px(0.), bottom: px(bottom) };
        Placing { view, picture, video_height: 1080, covered }
    }

    #[test]
    fn sizes_follow_the_picture_as_libvlc_s_do() {
        let picture = px(540.);
        let automatic = TextSize::Relative(RelativeSize::Automatic);
        assert_eq!(font_size(automatic, picture, 1080), px(33.75));
        assert_eq!(font_size(TextSize::Relative(RelativeSize::Larger), picture, 1080), px(90.));
        assert_eq!(font_size(TextSize::Pixels(64), picture, 1080), px(32.));
        assert_eq!(font_size(TextSize::Pixels(64), px(20.), 1080), px(MIN_FONT_SIZE));
    }

    #[test]
    fn the_band_sits_on_the_picture_or_in_the_bars_and_clears_the_bar() {
        let on_picture = Band::new(SubtitleArea::Picture, &placing(0.));
        assert_eq!(on_picture.bottom, px(681.25 - 22.5));
        assert_eq!(on_picture.width, px(900.));
        let in_bars = Band::new(SubtitleArea::Bars, &placing(0.));
        assert_eq!(in_bars.bottom, px(800. - 22.5));
        let covered = Band::new(SubtitleArea::Bars, &placing(100.));
        assert_eq!(covered.bottom, px(800. - 100. - BAR_GAP));
        let low_bar = Band::new(SubtitleArea::Picture, &placing(50.));
        assert_eq!(low_bar.bottom, on_picture.bottom, "a bar in the black bars covers nothing");
        let mut title = placing(0.);
        title.covered.top = px(40.);
        assert_eq!(Band::new(SubtitleArea::Bars, &title).top, px(40. + BAR_GAP));
    }

    #[test]
    fn cues_together_stack_without_overlapping() {
        let band = Band { center: px(500.), width: px(900.), top: px(20.), bottom: px(500.) };
        let heights = [px(40.), px(80.)];
        assert_eq!(stack(Placement::Bottom, &heights, &band), [px(460.), px(380.)]);
        assert_eq!(stack(Placement::Top, &heights, &band), [px(20.), px(60.)]);
        assert_eq!(stack(Placement::Middle, &heights, &band), [px(200.), px(240.)]);
    }
}
