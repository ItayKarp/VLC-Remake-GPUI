//! The miniplayer card (S4.10.3, `miniplayer-default`, `-variant3`, `-variant5`): the video, or
//! for audio its artwork tile, then a progress bar, the file's name, chips for its folder and
//! artist, and the queue toggle. Hovering the picture shows ✕ (stop, which closes the card),
//! pop-out (back to the full player), and Previous, Play/Pause and Next; once the media ended
//! they stay, with Replay in the middle.
//!
//! Clicking the progress bar seeks there. The folder chip goes to the file's folder in the
//! library and the artist chip to the artist.

use std::rc::Rc;
use std::time::Duration;

use gpui::{
    ClickEvent, Context, Div, Entity, MouseButton, MouseDownEvent, SharedString, Stateful,
    accesskit, div, prelude::*, px,
};

use super::dock::{CHIPS_HEIGHT, CardSize, NAME_HEIGHT, PAD, PROGRESS_GAP, PROGRESS_HEIGHT};
use crate::app::media::Media;
use crate::assets::icons;
use crate::db::metadata::MediaKind;
use crate::db::thumbnails::SizeClass;
use crate::engine::player::Player;
use crate::engine::values::SeekTarget;
use crate::library::folder_view::kind_by_extension;
use crate::main_window::MainWindow;
use crate::main_window::viewport;
use crate::navigation::route::{MusicRoute, Route};
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::button::IconButton;
use crate::ui::focus;
use crate::ui::icon::icon;
use crate::ui::metrics;
use crate::ui::metrics::MIN_TARGET;
use crate::ui::player_overlay::{self, Handlers, Reveal, TransportState, WindowButton};
use crate::ui::thumbnail::{Placeholder, ThumbnailTile};

/// What hovering the picture lights up together.
const GROUP: &str = "miniplayer-picture";
/// The name's and chips' text sizes, a chip's icon and the gap after it, and the gap between
/// chips.
const NAME_TEXT: f32 = 15.;
const CHIP_TEXT: f32 = 14.;
const CHIP_ICON: f32 = 18.;
const CHIP_ICON_GAP: f32 = 6.;
const CHIP_GAP: f32 = 16.;
const CHIP_MAX: f32 = 120.;
/// The queue toggle's size and icon.
const QUEUE_BUTTON: f32 = 28.;
const QUEUE_ICON: f32 = 20.;

impl MainWindow {
    /// The card for `player`'s media, `size` wide.
    pub(super) fn render_miniplayer_card(
        &self,
        player: &Entity<Player>,
        size: CardSize,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        let read = player.read(cx);
        let media = read.media().cloned();
        let artist = read.metadata().artist.clone();
        let fraction = progress(read.position(), read.duration());
        let picture = div()
            .id("miniplayer-picture")
            .relative()
            .w_full()
            .h(px(size.video_height))
            .group(GROUP)
            .child(self.render_picture(player, size, p, cx))
            .child(self.render_hover_controls(cx));
        div()
            .flex_none()
            .h(px(size.height()))
            .flex()
            .flex_col()
            .child(
                div()
                    .px(px(PAD))
                    .pt(px(PAD))
                    .child(picture)
                    .child(self.render_progress(fraction, p, cx)),
            )
            .child(name_line(media.as_ref(), p))
            .child(self.render_chips(media.as_ref(), artist, p, cx))
    }

    /// The video, or for audio-only media its artwork tile instead of a black frame.
    fn render_picture(
        &self,
        player: &Entity<Player>,
        size: CardSize,
        p: &Palette,
        cx: &Context<Self>,
    ) -> Div {
        let read = player.read(cx);
        let area = div().absolute().inset_0().rounded(px(metrics::RADIUS)).overflow_hidden();
        if let Some(video) = self.showing_video(cx).filter(|_| read.video_size().is_some()) {
            return area.child(viewport::rounded(video, p.video, p.panel));
        }
        let audio = read.media().and_then(Media::as_path).map(kind_by_extension);
        let placeholder =
            if audio == Some(MediaKind::Audio) { Placeholder::Song } else { Placeholder::Video };
        let tile = ThumbnailTile::new(SizeClass::Grid, placeholder)
            .size(size.width - 2. * PAD, size.video_height);
        area.bg(p.video).child(tile)
    }

    /// ✕ and pop-out in the top corners, and the transport in the middle: on hover, or always
    /// once the media ended.
    fn render_hover_controls(&self, cx: &mut Context<Self>) -> Div {
        let pop_out = WindowButton { icon: icons::POP_OUT, label: "Back to the player".into() };
        let handlers = Handlers {
            close: Rc::new(cx.listener(|this, _: &ClickEvent, window, cx| this.stop(window, cx))),
            window: Rc::new(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.leave_full_library(window, cx);
            })),
            previous: Rc::new(
                cx.listener(|this, _: &ClickEvent, window, cx| this.previous(window, cx)),
            ),
            play: Rc::new(
                cx.listener(|this, _: &ClickEvent, window, cx| this.play_pause(window, cx)),
            ),
            next: Rc::new(cx.listener(|this, _: &ClickEvent, window, cx| this.next(window, cx))),
        };
        let state = TransportState {
            reveal: Reveal::keyboard_inside(self.miniplayer.overlay_focused),
            ..self.transport_state(cx)
        };
        let overlay = player_overlay::overlay(GROUP, state, pop_out, &handlers, false);
        match &self.miniplayer.overlay_focus {
            Some(focus) => div().absolute().inset_0().track_focus(focus).child(overlay),
            None => overlay,
        }
    }

    /// The progress bar: a click seeks to where it lands.
    fn render_progress(&self, fraction: f32, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let bar = self.miniplayer.progress.clone();
        div()
            .id("miniplayer-progress")
            .relative()
            .mt(px(PROGRESS_GAP))
            .h(px(PROGRESS_HEIGHT))
            .rounded(px(PROGRESS_HEIGHT / 2.))
            .bg(p.sunken)
            .overflow_hidden()
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, cx.listener(Self::seek_on_progress))
            .child(bar.element())
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .h_full()
                    .w(gpui::relative(fraction))
                    .bg(p.accent_tint),
            )
    }

    /// Seeks to where the progress bar was pressed; the press doesn't drag the card.
    fn seek_on_progress(
        &mut self,
        event: &MouseDownEvent,
        _: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        let Some(bounds) = self.miniplayer.progress.bounds() else { return };
        let Some(player) = self.loaded_player(cx) else { return };
        let Some(duration) = player.read(cx).duration() else { return };
        let width = f32::from(bounds.size.width).max(1.);
        let fraction = (f32::from(event.position.x - bounds.left()) / width).clamp(0., 1.);
        self.seek_player(SeekTarget::Absolute(duration.mul_f32(fraction)), cx);
    }

    /// The folder and artist chips and, at the end, the queue toggle.
    fn render_chips(
        &self,
        media: Option<&Media>,
        artist: Option<String>,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        let folder = media.and_then(Media::as_path).and_then(|path| {
            let folder = path.parent()?.to_path_buf();
            Some((folder, kind_by_extension(path)))
        });
        let folder_chip = folder.map(|(folder, kind)| {
            let name: SharedString = Media::File(folder.clone()).display_name().into();
            chip("miniplayer-folder", icons::FOLDER, name, p).on_click(cx.listener(
                move |this, _: &ClickEvent, window, cx| {
                    this.show_library_folder(&folder, kind, window, cx);
                },
            ))
        });
        let artist_chip = artist.filter(|artist| !artist.is_empty()).map(|artist| {
            let route = Route::Music(MusicRoute::Artists(Some(artist.clone())));
            chip("miniplayer-artist", icons::ARTIST, artist.into(), p).on_click(cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    this.navigate(route.clone(), cx);
                },
            ))
        });
        let queue = IconButton::new("miniplayer-queue", icons::LIST, "Queue")
            .size(QUEUE_BUTTON, QUEUE_ICON)
            .selected(self.is_miniplayer_queue_open())
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.toggle_miniplayer_queue(window, cx);
            }));
        div()
            .h(px(CHIPS_HEIGHT))
            .px(px(PAD))
            .flex()
            .items_center()
            .gap(px(CHIP_GAP))
            .children(folder_chip)
            .children(artist_chip)
            .child(div().flex_1())
            .child(queue)
    }
}

/// How far `position` is through `duration`, from 0 to 1.
pub(in crate::main_window) fn progress(position: Duration, duration: Option<Duration>) -> f32 {
    match duration {
        Some(duration) if !duration.is_zero() => {
            (position.as_secs_f32() / duration.as_secs_f32()).clamp(0., 1.)
        }
        _ => 0.,
    }
}

/// The file's name under the progress bar.
fn name_line(media: Option<&Media>, p: &Palette) -> Div {
    let name: SharedString = media.map_or_default(Media::display_name).into();
    div()
        .h(px(NAME_HEIGHT))
        .px(px(PAD + PAD))
        .flex()
        .items_center()
        .text_size(text_px(NAME_TEXT))
        .text_color(p.text_muted)
        .child(div().min_w_0().truncate().child(name))
}

/// A chip: an icon and a name, which goes somewhere in the library. A tab stop that Enter
/// and Space press, at least [`MIN_TARGET`] tall (S9.1).
fn chip(id: &'static str, glyph: &'static str, name: SharedString, p: &Palette) -> Stateful<Div> {
    let hover = p.hover;
    let chip = div()
        .id(id)
        .tab_index(0)
        .key_context(focus::CONTROL)
        .role(accesskit::Role::Link)
        .aria_label(name.clone());
    focus::outline_ring(chip, p)
        .min_h(px(MIN_TARGET))
        .flex()
        .items_center()
        .gap(px(CHIP_ICON_GAP))
        .px(px(PAD / 2.))
        .rounded(px(metrics::RADIUS))
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .text_size(text_px(CHIP_TEXT))
        .text_color(p.text_muted)
        .tooltip(crate::ui::tooltip::text(name.clone(), None::<SharedString>))
        .child(icon(glyph, CHIP_ICON, p.icon))
        .child(div().max_w(px(CHIP_MAX)).truncate().child(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_is_a_fraction_of_a_known_length() {
        let secs = Duration::from_secs;
        assert!((progress(secs(30), Some(secs(120))) - 0.25).abs() < f32::EPSILON);
        assert!((progress(secs(200), Some(secs(120))) - 1.).abs() < f32::EPSILON);
        assert!(progress(secs(30), None).abs() < f32::EPSILON);
        assert!(progress(secs(30), Some(Duration::ZERO)).abs() < f32::EPSILON);
    }
}
