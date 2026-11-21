//! On-screen messages over the video (S2.5.7): a short note at the top right, such as the new
//! volume when it changes by key or wheel while the control bar is hidden. Not designed; it
//! takes the volume bubble's look (`volume/audio_value.png`), a little larger. S2.10 adds the
//! other messages.

use std::time::Duration;

use gpui::{Context, Div, SharedString, div, prelude::*, px};

use super::MainWindow;
use super::control_bar::clock;
use super::flash::Flash;
use crate::theme::Palette;

/// How long a message stays.
const SHOWN_FOR: Duration = Duration::from_millis(1500);
/// How far the message sits from the video's top and right edges.
const MARGIN: f32 = 24.;
/// The message's text size.
const TEXT_SIZE: f32 = 18.;

/// The message on the video, if any.
#[derive(Default)]
pub struct Osd {
    text: SharedString,
    flash: Flash,
}

impl MainWindow {
    /// Shows `text` over the video for a moment.
    pub(super) fn show_osd(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.osd.text = text.into();
        self.flash(|this| &mut this.osd.flash, SHOWN_FOR, cx);
    }

    /// The message, while it shows.
    pub(super) fn render_osd(&self, p: &Palette) -> Option<Div> {
        if !self.osd.flash.is_showing() {
            return None;
        }
        let message = div()
            .px(px(18.))
            .py(px(10.))
            .rounded(px(8.))
            .bg(p.panel)
            .shadow_lg()
            .text_size(px(TEXT_SIZE))
            .text_color(p.text_strong)
            .font_features(clock::tabular_digits())
            .child(self.osd.text.clone());
        Some(div().absolute().top(px(MARGIN)).right(px(MARGIN)).child(message))
    }
}
