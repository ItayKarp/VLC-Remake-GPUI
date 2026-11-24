//! Music mode's content (S8.1.1, S8.1.3, `player_music_01.png`): the cover in a large rounded
//! square in the middle, letterboxed when it isn't square, over its blurred copy stretched across
//! the whole area and dimmed, with "title - artist" under it. A file without a cover shows the
//! library's green music-note tile on black.

use gpui::{App, Div, Hsla, ImageSource, ObjectFit, black, div, hsla, img, prelude::*, px};

use super::MainWindow;
use super::cover::{Cover, Lookup};
use crate::app::media::Media;
use crate::db::thumbnails::SizeClass;
use crate::effects;
use crate::theme::text_px;
use crate::ui::thumbnail::{Placeholder, ThumbnailTile};

/// The cover's side as a share of the area's height (738 of 985 px in the design).
const ART_SHARE: f32 = 0.75;
/// The space between the cover and the caption as a share of the area's height.
const GAP_SHARE: f32 = 0.07;
/// The least room left beside the cover in a narrow window.
const SIDE_MARGIN: f32 = 32.;
/// The cover's corner radius.
const ART_RADIUS: f32 = 16.;
/// The caption's text size, and its colour: white over the dark backdrop in both themes.
const CAPTION_SIZE: f32 = 24.;
const CAPTION_COLOR: Hsla = hsla(0., 0., 1., 0.8);
/// The note under the caption while Settings → Video turned video off, and its size.
const VIDEO_OFF: &str = tr_noop!("Video is off in Settings → Video");
const NOTE_SIZE: f32 = 14.;
const NOTE_COLOR: Hsla = hsla(0., 0., 1., 0.5);
/// How dark the scrim over the backdrop is, so the cover and caption stand out.
const BACKDROP_DIM: f32 = 0.6;

/// The cover's side and the gap under it, in an area `width` by `height`.
fn placement(width: f32, height: f32) -> (f32, f32) {
    let side = (height * ART_SHARE).min(width - 2. * SIDE_MARGIN).max(0.);
    (side, height * GAP_SHARE)
}

impl MainWindow {
    /// The content area in music mode, `width` by `height` logical pixels.
    pub(super) fn render_music(&self, width: f32, height: f32, cx: &App) -> Div {
        let (side, gap) = placement(width, height);
        let cover = &self.music.cover;
        let video_off = !effects::effective(cx).prefs.video.enabled;
        let caption = div()
            .mt(px(gap))
            .max_w(px((width - 2. * SIDE_MARGIN).max(0.)))
            .truncate()
            .text_size(text_px(CAPTION_SIZE))
            .text_color(CAPTION_COLOR)
            .child(self.caption(cx));
        let note = video_off.then(|| {
            let text = crate::i18n::text(VIDEO_OFF);
            div().text_size(text_px(NOTE_SIZE)).text_color(NOTE_COLOR).child(text)
        });
        let column = div()
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .child(art(cover, side))
            .child(caption)
            .children(note);
        div()
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(black())
            .children(backdrop(cover))
            .child(column)
    }

    /// "title - artist": the title tag or else the file's name, and the artist tag if any.
    fn caption(&self, cx: &App) -> String {
        let Some(player) = self.loaded_player(cx) else { return String::new() };
        let player = player.read(cx);
        let tags = player.metadata();
        let title = tags.title.clone().filter(|title| !title.is_empty());
        let title = title.unwrap_or_else(|| player.media().map_or_default(Media::display_name));
        match tags.artist.as_deref().filter(|artist| !artist.is_empty()) {
            Some(artist) => tr!("{title} - {artist}", title, artist),
            None => title,
        }
    }
}

/// The blurred cover stretched over the whole area and dimmed, once there is one.
fn backdrop(cover: &Lookup) -> Option<Div> {
    let Lookup::Found(Cover { backdrop: Some(image), .. }) = cover else { return None };
    let stretched =
        img(ImageSource::Render(image.clone())).size_full().object_fit(ObjectFit::Cover);
    let scrim = div().absolute().inset_0().bg(black().opacity(BACKDROP_DIM));
    Some(div().absolute().inset_0().child(stretched).child(scrim))
}

/// The cover in its `side`-pixel square: the image letterboxed, the placeholder tile when there
/// is none, and nothing while it is looked up.
fn art(cover: &Lookup, side: f32) -> Div {
    let square = div().flex_none().size(px(side)).rounded(px(ART_RADIUS)).overflow_hidden();
    match cover {
        Lookup::Found(cover) => square.bg(black()).child(
            img(cover.path.clone())
                .size_full()
                .rounded(px(ART_RADIUS))
                .object_fit(ObjectFit::Contain),
        ),
        Lookup::Missing => {
            let tile = ThumbnailTile::new(SizeClass::Grid, Placeholder::Song).size(side, side);
            square.child(tile.radius(ART_RADIUS).always_dark())
        }
        Lookup::Looking => square,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cover_takes_three_quarters_of_the_height() {
        let (side, gap) = placement(1920., 985.);
        assert!((side - 738.75).abs() < 0.01);
        assert!((gap - 68.95).abs() < 0.01);
    }

    #[test]
    fn a_narrow_window_keeps_a_margin_beside_the_cover() {
        assert!((placement(400., 985.).0 - 336.).abs() < f32::EPSILON);
        assert!(placement(40., 985.).0.abs() < f32::EPSILON);
    }
}
