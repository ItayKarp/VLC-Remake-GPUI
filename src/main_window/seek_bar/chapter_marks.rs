//! Chapters on the seek bar (S8.2.2, not designed): a short tick across the track where each
//! chapter after the first starts, on the plain track and the waveform alike, and the name of the
//! chapter under the pointer in the hover card. The subtitle timeline, a close-up of a few
//! seconds, shows neither.

use std::time::Duration;

use gpui::{App, Div, div, prelude::*, px, relative};

use crate::main_window::MainWindow;
use crate::main_window::seeking::chapters;
use crate::theme::{Palette, blend};

/// A tick's width, and its distance from the track's top and bottom.
const TICK_WIDTH: f32 = 2.;
const TICK_INSET: f32 = 5.;
/// How much of the text a tick mixes into the track: as the waveform's upcoming bars.
const TICK_TINT: f32 = 0.35;

impl MainWindow {
    /// Where along the media each chapter but one starting at 0 starts, 0–1.
    pub(super) fn chapter_fractions(&self, duration: Duration, cx: &App) -> Vec<f32> {
        if duration.is_zero() {
            return Vec::new();
        }
        self.chapters(cx)
            .iter()
            .filter(|chapter| !chapter.start.is_zero())
            .map(|chapter| chapter.start.as_secs_f32() / duration.as_secs_f32())
            .collect()
    }

    /// The title of the chapter playing at `time`, if any.
    pub(super) fn chapter_title_at(&self, time: Duration, cx: &App) -> Option<String> {
        let mut listed = self.chapters(cx);
        let index = chapters::current(&listed, time)?;
        Some(listed.swap_remove(index).title)
    }
}

/// The ticks at `fractions` along the track, over the track's contents.
pub(super) fn ticks(p: &Palette, fractions: &[f32]) -> impl Iterator<Item = Div> {
    let color = blend(p.text, p.sunken, TICK_TINT);
    fractions.iter().map(move |&fraction| {
        div()
            .absolute()
            .top(px(TICK_INSET))
            .bottom(px(TICK_INSET))
            .left(relative(fraction))
            .ml(px(-TICK_WIDTH / 2.))
            .w(px(TICK_WIDTH))
            .rounded(px(1.))
            .bg(color)
    })
}
