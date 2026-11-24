//! The hover card (S2.3.2, `seek_thumbnail/thumbnail.png`): a 16:9 preview over the time it
//! shows, above the seek bar, following the pointer (or the handle while dragging) and kept
//! inside the window. The time shows at once; the picture arrives when the preview decoder has
//! it, a grey frame meanwhile or when there is none. Audio and network streams get the time
//! alone. Over a chapter, the chapter's name shows above the time (S8.2.2, not designed), the
//! time-only card widening to hold it.

use std::sync::Arc;

use gpui::{
    AnyElement, App, Context, Corners, Div, RenderImage, Window, black, canvas, div, prelude::*, px,
};

use super::geometry;
use super::preview::Picture;
use crate::main_window::MainWindow;
use crate::main_window::control_bar::clock;
use crate::theme::Palette;
use crate::theme::text_px;
use crate::video::layout;

/// The card's width with a picture, and its padding.
const CARD_WIDTH: f32 = 272.;
const PADDING: f32 = 10.;
/// The picture's height: 16:9 of the card's inner width.
const FRAME_HEIGHT: f32 = (CARD_WIDTH - 2. * PADDING) * 9. / 16.;
/// The card's width with the time alone, and with the time and a chapter's name.
const TIME_CARD_WIDTH: f32 = 96.;
const CHAPTER_CARD_WIDTH: f32 = 200.;
/// The chapter name's text size and its space above.
const CHAPTER_TEXT: f32 = 13.;
const CHAPTER_TOP: f32 = 8.;
/// The time row's height and text size.
const TIME_ROW: f32 = 34.;
const TIME_TEXT: f32 = 15.;
/// The card's distance from the bar, and from the window's edges.
const GAP: f32 = 8.;
const MARGIN: f32 = 8.;
/// The picture's aspect until the player knows the video size.
const WIDESCREEN: (u32, u32) = (16, 9);

impl MainWindow {
    /// The hover card, while the pointer is over the seek bar or the handle is dragged.
    pub(in crate::main_window) fn render_seek_card(
        &self,
        p: &Palette,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let seekable = self.seekable(cx)?;
        let fraction = self.seek_bar.pointed_fraction()?;
        let x = self.seek_bar.span()?.x_of(fraction);
        let time = self.seek_bar.time_at(fraction, seekable.duration);
        let chapter = self.chapter_title_at(time, cx);
        let (width, top) = card_shape(seekable.pictures, chapter.as_deref());
        let window_width = f32::from(window.viewport_size().width);
        let picture = seekable.pictures.then(|| self.preview_frame(p, cx));
        let card = div()
            .absolute()
            .left(px(geometry::card_left(x, width, window_width, MARGIN)))
            .bottom(px(self.control_bar_height() + GAP))
            .w(px(width))
            .px(px(PADDING))
            .pt(px(top))
            .rounded(px(12.))
            .bg(p.popover)
            .border_1()
            .border_color(p.divider)
            .shadow_lg()
            .flex()
            .flex_col()
            .items_center()
            .children(picture)
            .children(chapter.map(|title| chapter_name(title, seekable.pictures, p)))
            .child(
                div()
                    .h(px(TIME_ROW))
                    .flex()
                    .items_center()
                    .text_size(text_px(TIME_TEXT))
                    .text_color(p.text_muted)
                    .child(clock::format(time)),
            );
        Some(card.into_any_element())
    }

    /// The 16:9 frame: the newest preview letterboxed on black, or a grey placeholder.
    fn preview_frame(&self, p: &Palette, cx: &App) -> Div {
        let frame = div().w_full().h(px(FRAME_HEIGHT)).rounded(px(8.)).overflow_hidden();
        let Some(Picture::Frame(image)) = self.seek_bar.previews.picture() else {
            return frame.bg(p.well);
        };
        let aspect = self
            .loaded_player(cx)
            .and_then(|player| player.read(cx).video_size())
            .map_or(WIDESCREEN, |size| (size.width, size.height));
        frame.bg(black()).child(picture(image.clone(), aspect))
    }
}

/// The card's width and the space above its contents: wide with a `picture`, a little wider for a
/// chapter's name, else just wide enough for the time.
fn card_shape(picture: bool, chapter: Option<&str>) -> (f32, f32) {
    match (picture, chapter) {
        (true, _) => (CARD_WIDTH, PADDING),
        (false, Some(_)) => (CHAPTER_CARD_WIDTH, CHAPTER_TOP),
        (false, None) => (TIME_CARD_WIDTH, 0.),
    }
}

/// The chapter's name, cut short to fit; below the picture when there is one.
fn chapter_name(title: String, below_picture: bool, p: &Palette) -> Div {
    div()
        .w_full()
        .pt(px(if below_picture { CHAPTER_TOP } else { 0. }))
        .text_size(text_px(CHAPTER_TEXT))
        .text_color(p.text)
        .text_center()
        .truncate()
        .child(title)
}

/// Paints `image` letterboxed at `aspect` over its bounds.
fn picture(image: Arc<RenderImage>, aspect: (u32, u32)) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            let placed = layout::letterbox(bounds, aspect, window.scale_factor());
            let painted = window.paint_image(placed, placed, Corners::default(), image, 0, false);
            if let Err(error) = painted {
                tracing::warn!("seek preview: couldn't paint a picture: {error:#}");
            }
        },
    )
    .size_full()
}
