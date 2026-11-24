//! Show Statistics (S8.13.2): the settings panel's toggle shows, over the video's top left, the
//! codecs, resolution, bitrate, dropped frames, buffer and whether the GPU decodes. Not
//! designed: it takes the on-screen messages' bubble, in smaller type, below the place a message
//! shows, so a message never covers it and it doesn't move when one comes or goes. It lasts the
//! session and is never saved.
//!
//! The counts are read from the backend once a second while it shows. On a live stream they
//! keep coming; only what the stream doesn't say shows "—".

pub(super) mod rows;

use std::time::Duration;

use gpui::{AnyElement, App, AsyncApp, Context, Div, Task, WeakEntity, div, prelude::*, px};

use super::MainWindow;
use super::control_bar::clock;
use super::osd;
use crate::engine::tracks::TrackKind;
use crate::i18n::DirectionalStyle;
use crate::theme::Palette;
use crate::theme::text_px;
use rows::Sources;

/// How often the counts are read.
const READ_EVERY: Duration = Duration::from_secs(1);
/// The space between the message's place and the overlay, the overlay's text size, and the
/// space between a label and its value.
const BELOW_MESSAGE: f32 = 8.;
const TEXT_SIZE: f32 = 13.;
const COLUMN_GAP: f32 = 16.;

/// Whether the overlay shows, and the loop reading the counts meanwhile.
#[derive(Default)]
pub(super) struct StatisticsOverlay {
    shown: bool,
    _reading: Option<Task<()>>,
}

impl MainWindow {
    /// Whether Show Statistics is on.
    pub(super) fn shows_statistics(&self) -> bool {
        self.statistics.shown
    }

    /// Turns Show Statistics on or off.
    pub(super) fn set_show_statistics(&mut self, shown: bool, cx: &mut Context<Self>) {
        let reading = shown.then(|| cx.spawn(async move |this, cx| keep_reading(this, cx).await));
        self.statistics = StatisticsOverlay { shown, _reading: reading };
        cx.notify();
    }

    /// Asks the player for its counts; returns whether the overlay still shows.
    fn read_statistics(&mut self, cx: &mut Context<Self>) -> bool {
        if let Some(player) = self.loaded_player(cx) {
            player.update(cx, |player, _| player.read_statistics());
        }
        cx.notify();
        self.statistics.shown
    }

    /// The overlay, while on and media is open.
    pub(super) fn render_statistics(&self, p: &Palette, cx: &App) -> Option<AnyElement> {
        if !self.statistics.shown {
            return None;
        }
        let player = self.loaded_player(cx)?;
        let player = player.read(cx);
        let tracks = player.tracks();
        let playing = |kind| tracks.selected(kind).and_then(|id| tracks.get(id));
        let rows = rows::rows(&Sources {
            video: playing(TrackKind::Video),
            audio: playing(TrackKind::Audio),
            size: player.video_size(),
            buffer: player.buffer_fill(),
            counts: player.statistics(),
        });
        let (labels, values): (Vec<_>, Vec<_>) = rows.into_iter().unzip();
        let table = div()
            .flex_row_start()
            .gap(px(COLUMN_GAP))
            .child(column().text_color(p.text_muted).children(labels.into_iter().map(div_of)))
            .child(column().children(values.into_iter().map(div_of)));
        let bubble = div()
            .px(px(14.))
            .py(px(10.))
            .rounded(px(8.))
            .bg(p.panel)
            .shadow_lg()
            .text_size(text_px(TEXT_SIZE))
            .text_color(p.text_strong)
            .font_features(clock::tabular_digits())
            .child(table);
        let top = osd::MARGIN + osd::HEIGHT + BELOW_MESSAGE;
        Some(div().absolute().top(px(top)).left(px(osd::MARGIN)).child(bubble).into_any_element())
    }
}

/// Reads the counts every [`READ_EVERY`] until the window goes away or the overlay hides.
async fn keep_reading(this: WeakEntity<MainWindow>, cx: &mut AsyncApp) {
    while let Ok(true) = this.update(cx, MainWindow::read_statistics) {
        cx.background_executor().timer(READ_EVERY).await;
    }
}

/// A cell of the table.
fn div_of(text: impl Into<gpui::SharedString>) -> Div {
    div().child(text.into())
}

/// A column of the table.
fn column() -> Div {
    div().flex_row_start().flex_col().gap(px(2.))
}
