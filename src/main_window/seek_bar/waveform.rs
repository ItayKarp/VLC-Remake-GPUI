//! The waveform seek bar (S7.8.2, S7.8.3, `controls-waveform.png`): with the Audio Visualizer set
//! to Waveform, or in music mode (S8.1.2, `controls-music.png`), a local file with a known length
//! shows its loudness along the bar, a thin bar every few pixels, those already played in the
//! accent and the rest grey.
//!
//! The profile is made off the UI thread ([`WaveformJob`]) and drawn as it arrives: the part not
//! yet decoded shows as the plain track, so a long file fills in from its start. Network streams
//! and live media keep the plain bar, as does the subtitle timeline while it shows.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt as _;
use gpui::{
    App, AsyncApp, Bounds, Context, Div, Hsla, Task, WeakEntity, canvas, div, fill, point,
    prelude::*, px, size,
};

use super::render::handle;
use crate::engine::waveform::{Peaks, WaveformEvent, WaveformJob, WaveformReceiver};
use crate::main_window::MainWindow;
use crate::settings;
use crate::theme::{Palette, blend};

/// The space from one bar to the next, a bar's width, and the shortest bar, in pixels.
const BAR_PITCH: f32 = 3.;
const BAR_WIDTH: f32 = 1.5;
const MIN_BAR: f32 = 1.;
/// The space above and below the tallest bar.
const BAR_INSET: f32 = 4.;
/// How much of the text an upcoming bar mixes into the track.
const UPCOMING_TINT: f32 = 0.35;

/// The profile the seek bar draws, and the work making it.
#[derive(Default)]
pub(super) struct WaveformState {
    /// The file it is for.
    media: Option<PathBuf>,
    /// The profile so far; `None` until the first part arrives, or when there can be none.
    peaks: Option<Arc<Peaks>>,
    /// Stops the work when dropped.
    _job: Option<WaveformJob>,
    _follow: Option<Task<()>>,
}

impl WaveformState {
    /// The profile so far, if any.
    pub(super) fn peaks(&self) -> Option<Arc<Peaks>> {
        self.peaks.clone()
    }
}

impl MainWindow {
    /// Starts making the loaded file's profile when the seek bar should show it and doesn't
    /// have it, and forgets it when it shouldn't.
    pub(in crate::main_window) fn sync_waveform(&mut self, cx: &mut Context<Self>) {
        let wanted = self.waveform_source(cx);
        let state = &mut self.seek_bar.waveform;
        if wanted.as_ref().map(|(path, _)| path) == state.media.as_ref() {
            return;
        }
        *state = WaveformState::default();
        cx.notify();
        let Some((path, length)) = wanted else { return };
        let (job, receiver) = WaveformJob::start(path.clone(), length);
        let follow = cx.spawn(async move |this, cx| follow(receiver, this, cx).await);
        *state = WaveformState {
            media: Some(path),
            peaks: None,
            _job: Some(job),
            _follow: Some(follow),
        };
    }

    /// The loaded file and its length, while the Audio Visualizer is set to Waveform or music
    /// mode plays (S8.1.2).
    fn waveform_source(&self, cx: &App) -> Option<(PathBuf, Duration)> {
        if !settings::get(cx).prefs.audio.visualizer.shows_waveform() && !self.is_music_mode() {
            return None;
        }
        let player = self.loaded_player(cx)?.read(cx);
        let length = player.duration().filter(|length| !length.is_zero())?;
        Some((player.media()?.as_path()?.to_path_buf(), length))
    }
}

/// Takes the job's reports until it ends or the window is gone.
async fn follow(mut receiver: WaveformReceiver, this: WeakEntity<MainWindow>, cx: &mut AsyncApp) {
    while let Some(event) = receiver.next().await {
        let peaks = match event {
            WaveformEvent::Progress(peaks) | WaveformEvent::Done(peaks) => Some(peaks),
            WaveformEvent::Unavailable => None,
        };
        let taken = this.update(cx, |this, cx| {
            this.seek_bar.waveform.peaks = peaks;
            cx.notify();
        });
        if taken.is_err() {
            break;
        }
    }
}

/// The inside of the track: a bar for each stretch decoded, played up to `played` (0–1), and the
/// handle there.
pub(super) fn bars(p: &Palette, peaks: Arc<Peaks>, played: f32) -> Div {
    let colors = [p.accent, blend(p.text, p.sunken, UPCOMING_TINT)];
    let drawn = canvas(
        |_, _, _| {},
        move |bounds, (), window, _| paint_bars(bounds, &peaks, played, colors, window),
    )
    .size_full();
    div()
        .absolute()
        .inset_0()
        .rounded(px(3.))
        .overflow_hidden()
        .child(drawn)
        .child(handle(p, played))
}

/// Paints a bar every [`BAR_PITCH`] across `bounds`, as tall as the loudest stretch under it, in
/// `colors`' first (played) or second (upcoming); stretches not decoded yet get none.
fn paint_bars(
    bounds: Bounds<gpui::Pixels>,
    peaks: &Peaks,
    played: f32,
    colors: [Hsla; 2],
    window: &mut gpui::Window,
) {
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    let count = (width / BAR_PITCH).floor() as usize;
    if count == 0 {
        return;
    }
    let tallest = (height - 2. * BAR_INSET).max(MIN_BAR);
    for index in 0..count {
        let from = index as f32 / count as f32;
        let to = (index + 1) as f32 / count as f32;
        let Some(level) = peaks.level_between(from, to) else { break };
        let bar = (level * tallest).max(MIN_BAR);
        let x = f32::from(bounds.left()) + index as f32 * BAR_PITCH;
        let y = f32::from(bounds.top()) + (height - bar) / 2.;
        let color = if from < played { colors[0] } else { colors[1] };
        let shape = Bounds::new(point(px(x), px(y)), size(px(BAR_WIDTH), px(bar)));
        window.paint_quad(fill(shape, color));
    }
}
