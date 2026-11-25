//! On-screen messages over the video (S2.5.7, S2.10.2): a short note in the video's top-left
//! corner for the volume (while the control bar is hidden), seeks, the speed, track changes and
//! T (the position and length). Not designed: it takes the volume bubble's look
//! (`volume/audio_value.png`), a little larger. Each message fades in, unless the system or
//! Settings ask to reduce motion; then it simply appears. A frame step slower than a moment
//! shows a spinner there instead.

pub(super) mod messages;

use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyElement, App, Context, Div, SharedString, Transformation, div,
    percentage, prelude::*, px, svg,
};

use super::MainWindow;
use super::control_bar::clock;
use super::flash::Flash;
use crate::assets::icons;
use crate::theme::Palette;
use crate::theme::text_px;

/// How long a message stays.
const SHOWN_FOR: Duration = Duration::from_millis(1500);
/// How long a message takes to fade in.
const FADE_IN: Duration = Duration::from_millis(120);
/// How far the message sits from the video's top and left edges.
pub(super) const MARGIN: f32 = 24.;
/// The message's height, fixed so what sits below it (the statistics, S8.13.2) stays put.
pub(super) const HEIGHT: f32 = 50.;
/// The message's text size.
const TEXT_SIZE: f32 = 18.;
/// The frame step spinner's size.
const SPINNER_SIZE: f32 = 18.;
/// How long the spinner takes to turn once.
const SPIN: Duration = Duration::from_millis(900);

/// The message on the video, if any.
#[derive(Default)]
pub struct Osd {
    text: SharedString,
    flash: Flash,
    /// Counts messages, so each one fades in afresh.
    shown: u64,
}

impl MainWindow {
    /// Shows `text` over the video for a moment.
    pub(super) fn show_osd(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.osd.text = text.into();
        self.osd.shown += 1;
        self.flash(|this| &mut this.osd.flash, SHOWN_FOR, cx);
    }

    /// The last message shown, for tests of what says what.
    #[cfg(test)]
    pub(super) fn osd_text(&self) -> &str {
        &self.osd.text
    }

    /// The message, or the frame step spinner, while either shows.
    pub(super) fn render_osd(&self, p: &Palette, cx: &App) -> Option<AnyElement> {
        let reduce_motion = cx.reduce_motion();
        let content = if self.is_frame_step_slow(cx) {
            stepping(p, reduce_motion)
        } else if self.osd.flash.is_showing() {
            div().child(self.osd.text.clone())
        } else {
            return None;
        };
        let message = bubble(p).child(content);
        let placed = div().absolute().top(px(MARGIN)).left(px(MARGIN)).child(message);
        if reduce_motion {
            return Some(placed.into_any_element());
        }
        let fade = Animation::new(FADE_IN);
        Some(
            placed
                .with_animation(("osd", self.osd.shown), fade, Styled::opacity)
                .into_any_element(),
        )
    }
}

/// The message's surface.
fn bubble(p: &Palette) -> Div {
    div()
        .h(px(HEIGHT))
        .flex()
        .items_center()
        .px(px(18.))
        .rounded(px(8.))
        .bg(p.panel)
        .shadow_lg()
        .text_size(text_px(TEXT_SIZE))
        .text_color(p.text_strong)
        .font_features(clock::tabular_digits())
}

/// A spinner and "Stepping…", for a frame step that takes a while; the spinner stands still
/// when motion is reduced.
fn stepping(p: &Palette, reduce_motion: bool) -> Div {
    let spinner = svg().path(icons::SPINNER).size(px(SPINNER_SIZE)).text_color(p.text_strong);
    let spinner = if reduce_motion {
        spinner.into_any_element()
    } else {
        spinner
            .with_animation("frame-step", Animation::new(SPIN).repeat(), |spinner, turned| {
                spinner.with_transformation(Transformation::rotate(percentage(turned)))
            })
            .into_any_element()
    };
    div().flex().items_center().gap(px(10.)).child(spinner).child(tr!("Stepping…"))
}
