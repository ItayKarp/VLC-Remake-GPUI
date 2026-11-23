//! Drawing the seek bar: the dark track, the played part in a dark accent, the accent handle,
//! the buffered band for network streams (S2.3.5), the LIVE chip, and its accessible slider
//! description (S2.3.6). While it is a subtitle timeline (S6.6) the track shows the window's
//! lines instead ([`super::timeline_track`]), between the timeline's buttons, and the slider still
//! speaks of the media's current time and length.

use std::time::Duration;

use gpui::{
    AnyElement, App, Context, CursorStyle, Div, Entity, MouseButton, MouseMoveEvent, MouseUpEvent,
    Stateful, Window, accesskit, canvas, div, prelude::*, px, relative,
};

use super::geometry;
use super::subtitle_timeline::Frame;
use super::timeline::Visible;
use super::timeline_track;
use crate::engine::player::Player;
use crate::engine::state::PlaybackState;
use crate::main_window::MainWindow;
use crate::main_window::control_bar::{SEEK_MIN, clock};
use crate::settings::{self, SkipLength};
use crate::theme::{Palette, blend};
use crate::ui::focus;

/// The seek bar's height, and its handle's width.
const SEEK_HEIGHT: f32 = 26.;
const HANDLE_WIDTH: f32 = 8.;
/// How much of the accent the played part mixes into the track.
const PLAYED_TINT: f32 = 0.23;
/// How much lighter than the track the buffered band is.
const BUFFERED_TINT: f32 = 0.08;
/// The LIVE chip's text size.
const LIVE_TEXT: f32 = 11.;

/// What the bar shows, read from the player once per frame.
#[derive(Clone, Copy, Debug, Default)]
struct Shown {
    /// Where the handle is, 0–1.
    fraction: f32,
    /// The position and length, when the length is known.
    times: Option<(Duration, Duration)>,
    /// Where the buffered band ends, for network streams.
    buffered: Option<f32>,
    /// A stream without a length.
    live: bool,
    /// Whether pressing it seeks.
    seekable: bool,
}

impl MainWindow {
    /// The seek bar item of the control bar.
    pub(in crate::main_window) fn render_seek_bar(
        &self,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let timeline = self.timeline_frame(cx);
        let shown = self.shown_seek(timeline.as_ref().map(|frame| frame.visible), cx);
        let inside = match &timeline {
            Some(frame) => {
                let handle = self.timeline_handle(frame, shown);
                timeline_track::lines(p, frame, handle, PLAYED_TINT)
            }
            None => track(p, shown),
        };
        let area = Self::track_area(shown, cx)
            .child(self.seek_bar.track.element())
            .child(inside)
            .children(shown.live.then(|| live_chip(p)));
        let bar = div()
            .id("seek-bar")
            .relative()
            .flex_1()
            .min_w(px(SEEK_MIN))
            .h(px(SEEK_HEIGHT))
            .flex()
            .items_center()
            .rounded(px(4.))
            .bg(p.sunken)
            .border_1()
            .border_color(p.divider)
            .track_focus(&self.seek_bar.focus)
            .children(self.drag_listener(cx));
        let bar = match timeline {
            Some(_) => self.timeline_ends(bar, area, p, cx),
            None => bar.child(area),
        };
        accessible(focus::ring(bar, p), shown, cx).into_any_element()
    }

    /// The part of the bar that seeks: all of it, or the timeline between its buttons.
    fn track_area(shown: Shown, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("seek-track")
            .relative()
            .flex_1()
            .h_full()
            .cursor(if shown.seekable {
                CursorStyle::PointingHand
            } else {
                CursorStyle::OperationNotAllowed
            })
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                this.hover_seek_bar(event.position.x, cx);
            }))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if !hovered {
                    this.leave_seek_bar(cx);
                }
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::start_scrub))
    }

    /// Where the timeline's handle is: where it is dragged, or the playhead while it is in the
    /// window.
    fn timeline_handle(&self, frame: &Frame, shown: Shown) -> Option<f32> {
        let showing = self.seek_bar.scrub.is_some() || frame.visible.covers(frame.position);
        showing.then_some(shown.fraction)
    }

    /// What the bar shows; `timeline` is the subtitle timeline's window while it shows.
    fn shown_seek(&self, timeline: Option<Visible>, cx: &App) -> Shown {
        let Some(player) = self.loaded_player(cx) else { return Shown::default() };
        let read = player.read(cx);
        let seekable = self.seekable(cx).is_some();
        let Some(duration) = read.duration() else {
            // While opening, the length simply isn't known yet.
            let state = read.state();
            let live = state.is_active() && state != PlaybackState::Opening;
            return Shown { live, ..Shown::default() };
        };
        let position = read.position();
        let fraction = self.seek_bar.scrub.map_or_else(
            || {
                timeline.map_or(geometry::fraction_of(position, duration), |shown| {
                    shown.fraction_of(position)
                })
            },
            geometry::Scrub::fraction,
        );
        let buffered = buffered(&player, cx)
            .filter(|_| timeline.is_none())
            .map(|fill| geometry::buffered_end(position, fill, duration));
        Shown { fraction, times: Some((position, duration)), buffered, live: false, seekable }
    }

    /// While dragging, follows the pointer anywhere in the window (and past its edges).
    fn drag_listener(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.seek_bar.is_scrubbing() {
            return None;
        }
        let entity = cx.entity();
        let listener =
            canvas(|_, _, _| {}, move |_, (), window, _| listen_for_drag(entity, window));
        Some(listener.absolute().size_0().into_any_element())
    }
}

/// The buffer level of a network stream, while it plays.
fn buffered(player: &Entity<Player>, cx: &App) -> Option<f32> {
    let read = player.read(cx);
    let network = read.media().is_some_and(|media| media.as_path().is_none());
    (network && read.state().is_active()).then(|| read.buffer_fill())
}

/// The inside of the bar: the buffered band, the played part and the handle.
fn track(p: &Palette, shown: Shown) -> Div {
    let track = p.sunken;
    let band = shown.buffered.filter(|end| *end > shown.fraction).map(|end| {
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(relative(shown.fraction))
            .w(relative(end - shown.fraction))
            .bg(blend(p.text, track, BUFFERED_TINT))
    });
    let handle = shown.times.map(|_| handle(p, shown.fraction));
    div()
        .absolute()
        .inset_0()
        .rounded(px(3.))
        .overflow_hidden()
        .children(band)
        .child(div().h_full().w(relative(shown.fraction)).bg(blend(p.accent, track, PLAYED_TINT)))
        .children(handle)
}

/// The accent handle at `fraction` along the track.
pub(super) fn handle(p: &Palette, fraction: f32) -> Div {
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .left(relative(fraction))
        // Keeps the handle inside the track at both ends.
        .ml(px(-HANDLE_WIDTH * fraction))
        .w(px(HANDLE_WIDTH))
        .rounded(px(2.))
        .bg(p.accent)
}

/// The chip shown on a stream without a length (not designed).
fn live_chip(p: &Palette) -> Div {
    div()
        .absolute()
        .left(px(6.))
        .top(px(5.))
        .px(px(6.))
        .rounded(px(3.))
        .bg(p.danger)
        .text_size(px(LIVE_TEXT))
        .font_weight(gpui::FontWeight::BOLD)
        .text_color(gpui::white())
        .child("LIVE")
}

/// Describes the bar to screen readers as a slider over the media's seconds.
fn accessible(bar: Stateful<Div>, shown: Shown, cx: &App) -> Stateful<Div> {
    let bar = bar.role(accesskit::Role::Slider).aria_label("Seek");
    let Some((position, duration)) = shown.times else {
        let value = if shown.live { "Live" } else { clock::UNKNOWN };
        return bar.aria_value(value);
    };
    let step = settings::get(cx).prefs.controls.skip(SkipLength::Short);
    bar.aria_numeric_value(position.as_secs_f64())
        .aria_min_numeric_value(0.)
        .aria_max_numeric_value(duration.as_secs_f64())
        .aria_numeric_value_step(step.as_secs_f64())
        .aria_value(format!("{} of {}", clock::format(position), clock::format(duration)))
        .aria_orientation(accesskit::Orientation::Horizontal)
}

/// Sends every pointer move and the release to the window, wherever the pointer is.
fn listen_for_drag(main: Entity<MainWindow>, window: &mut Window) {
    let moved = main.clone();
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
        if phase.bubble() && event.dragging() {
            let fine = event.modifiers.shift;
            moved.update(cx, |this, cx| this.scrub_to(event.position, fine, cx));
        }
    });
    window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
        if phase.bubble() {
            main.update(cx, MainWindow::end_scrub);
        }
    });
}
