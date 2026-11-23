//! The seek bar (S2.3, `controls-default.png`): click to jump, drag to scrub (Shift for a
//! tenth of the speed), and a hover card with a preview picture and the time
//! (`seek_thumbnail/thumbnail.png`).
//!
//! A drag moves only the handle and the card; the player seeks once, on release. Pictures come
//! from a second, muted decoder ([`preview`]), never from seeking the main player. Media
//! without a known length is live: the handle is hidden, seeking is off and a LIVE chip shows.
//! Media that can't seek shows a "not allowed" cursor and ignores clicks.
//!
//! With Show Subtitle Timeline on Seekbar on, it shows a close-up of the time around the playhead
//! with the lines of the subtitle showing instead ([`subtitle_timeline`], S6.6). With the Audio
//! Visualizer set to Waveform, it shows the file's loudness along it ([`waveform`], S7.8.2).
//! Chapters are marked along it, and the hover card names the one pointed at ([`chapter_marks`],
//! S8.2.2).

mod card;
mod chapter_marks;
mod cue_drag;
mod geometry;
mod preview;
mod render;
mod subtitle_timeline;
mod timeline;
mod timeline_track;
mod waveform;

use std::cell::Cell;
use std::time::Duration;

use futures::StreamExt as _;
use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, MouseDownEvent, Pixels, Point, Window,
};

use self::cue_drag::CueDrag;
use self::geometry::{Scrub, Span};
use self::preview::Previews;
use self::timeline::{Timeline, Visible};
use self::waveform::WaveformState;
use super::MainWindow;
use crate::app::media::Media;
use crate::engine::player::Player;
use crate::engine::preview::{self as engine_preview, PreviewReceiver};
use crate::engine::state::PlaybackState;
use crate::engine::values::SeekTarget;
use crate::ui::popover::BoundsTracker;

/// How far before the end the last preview picture is taken.
const END_MARGIN: Duration = Duration::from_millis(500);

/// The seek bar's own state.
pub struct SeekBar {
    track: BoundsTracker,
    focus: FocusHandle,
    /// The pointer's window x while it is over the bar.
    hover_x: Option<f32>,
    /// The drag in progress.
    scrub: Option<Scrub>,
    previews: Previews,
    /// The subtitle timeline's window; drawing moves it along with the playhead.
    timeline: Cell<Timeline>,
    /// The window the timeline last showed, or `None` while the bar shows the whole media.
    shown_timeline: Cell<Option<Visible>>,
    /// A subtitle line being dragged on the timeline (S6.6.3).
    cue_drag: Option<CueDrag>,
    /// The loaded file's loudness, while the bar shows it (S7.8.2).
    waveform: WaveformState,
}

impl SeekBar {
    /// A seek bar that is a tab stop of `cx`'s window.
    pub fn new(cx: &mut App) -> Self {
        Self {
            track: BoundsTracker::new(),
            focus: cx.focus_handle().tab_stop(true),
            hover_x: None,
            scrub: None,
            previews: Previews::default(),
            timeline: Cell::default(),
            shown_timeline: Cell::new(None),
            cue_drag: None,
            waveform: WaveformState::default(),
        }
    }

    /// Whether the handle or a subtitle line is being dragged.
    pub(super) fn is_scrubbing(&self) -> bool {
        self.scrub.is_some() || self.cue_drag.is_some()
    }

    /// The track's span in the last frame.
    fn span(&self) -> Option<Span> {
        let bounds = self.track.bounds()?;
        Some(Span { left: f32::from(bounds.left()), width: f32::from(bounds.size.width) })
    }

    /// Where along the track the card points: the handle while dragging, else the pointer.
    fn pointed_fraction(&self) -> Option<f32> {
        if let Some(scrub) = self.scrub {
            return Some(scrub.fraction());
        }
        Some(self.span()?.fraction_at(self.hover_x?))
    }

    /// The time at `fraction` along the track: of the timeline's window while it shows, else of
    /// the media.
    fn time_at(&self, fraction: f32, duration: Duration) -> Duration {
        match self.shown_timeline.get() {
            Some(shown) => shown.time_at(fraction),
            None => geometry::time_at(fraction, duration),
        }
    }

    /// Where seeking to `fraction` along the track goes.
    fn target_at(&self, fraction: f32) -> SeekTarget {
        match self.shown_timeline.get() {
            Some(shown) => SeekTarget::Absolute(shown.time_at(fraction)),
            None => SeekTarget::Fraction(fraction),
        }
    }
}

/// What seeking needs to know about the loaded media; only media that can seek has it.
struct Seekable {
    player: Entity<Player>,
    duration: Duration,
    media: Media,
    /// Whether the card shows pictures: local video. Audio shows the time alone, and a network
    /// stream isn't opened a second time for previews.
    pictures: bool,
}

impl MainWindow {
    /// The loaded media, if it can seek now.
    fn seekable(&self, cx: &App) -> Option<Seekable> {
        let player = self.loaded_player(cx)?;
        let read = player.read(cx);
        let can_seek = read.state().is_active() && read.is_seekable() != Some(false);
        let duration = read.duration().filter(|_| can_seek)?;
        let media = read.media()?.clone();
        let pictures = read.video_size().is_some() && media.as_path().is_some();
        Some(Seekable { player, duration, media, pictures })
    }

    /// Forgets the drag and the previews: new media is opening.
    pub(super) fn reset_seek_bar(&mut self, state: PlaybackState) {
        if state == PlaybackState::Opening {
            self.seek_bar.scrub = None;
            self.seek_bar.cue_drag = None;
            self.seek_bar.previews.forget();
            self.seek_bar.timeline.get_mut().restart();
        }
    }

    /// Frees replaced preview pictures; called as the window draws.
    pub(super) fn release_previews(&mut self, window: &mut Window) {
        self.seek_bar.previews.release_retired(window);
    }

    fn hover_seek_bar(&mut self, x: Pixels, cx: &mut Context<Self>) {
        self.seek_bar.hover_x = Some(f32::from(x));
        self.ask_preview(cx);
        cx.notify();
    }

    fn leave_seek_bar(&mut self, cx: &mut Context<Self>) {
        self.seek_bar.hover_x = None;
        cx.notify();
    }

    /// Starts a drag where the bar was pressed: the handle jumps there.
    fn start_scrub(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.seek_bar.focus.focus(window, cx);
        let Some(span) = self.seek_bar.span() else { return };
        if self.seekable(cx).is_none() || self.press_cue(event.position.x, span, cx) {
            return;
        }
        self.seek_bar.scrub = Some(Scrub::start(f32::from(event.position.x), span));
        self.ask_preview(cx);
        cx.notify();
    }

    /// Follows the pointer during a drag, wherever it is: the handle, or a dragged subtitle line.
    fn scrub_to(&mut self, position: Point<Pixels>, fine: bool, cx: &mut Context<Self>) {
        let Some(span) = self.seek_bar.span() else { return };
        if self.drag_cue(position.x, span, cx) {
            return;
        }
        let Some(scrub) = &mut self.seek_bar.scrub else { return };
        scrub.move_to(f32::from(position.x), fine, span);
        self.ask_preview(cx);
        cx.notify();
    }

    /// Ends a drag: the player seeks to where the handle is, or the dragged line is let go.
    fn end_scrub(&mut self, cx: &mut Context<Self>) {
        if self.release_cue(cx) {
            return;
        }
        let Some(scrub) = self.seek_bar.scrub.take() else { return };
        if let Some(Seekable { player, .. }) = self.seekable(cx) {
            let target = self.seek_bar.target_at(scrub.fraction());
            self.seek_bar.timeline.get_mut().unpin();
            player.update(cx, |player, cx| player.seek(target, cx));
        }
        cx.notify();
    }

    /// Asks for a picture of the time the card points at, creating the decoder first if needed.
    fn ask_preview(&mut self, cx: &mut Context<Self>) {
        let Some(seekable) = self.seekable(cx).filter(|seekable| seekable.pictures) else {
            return;
        };
        let Some(fraction) = self.seek_bar.pointed_fraction() else { return };
        if self.seek_bar.previews.needs_decoder() {
            Self::create_preview_decoder(cx);
            return;
        }
        // A picture of the very end would run the decoder past it: show the last moment.
        let last = seekable.duration.saturating_sub(END_MARGIN);
        let time = self.seek_bar.time_at(fraction, seekable.duration).min(last);
        self.seek_bar.previews.want(&seekable.media, time, seekable.duration);
    }

    /// Creates the preview decoder off the UI thread (it starts a libVLC instance), then asks
    /// again; its answers redraw the card as they arrive.
    fn create_preview_decoder(cx: &mut Context<Self>) {
        let (sender, receiver) = engine_preview::channel();
        let created = cx.background_spawn(async move { engine_preview::default_decoder(sender) });
        cx.spawn(async move |this, cx| {
            let decoder = created.await;
            let ready = this.update(cx, |this, cx| {
                this.seek_bar.previews.set_decoder(decoder);
                this.ask_preview(cx);
            });
            if ready.is_ok() {
                follow_answers(receiver, this, cx).await;
            }
        })
        .detach();
    }
}

/// Hands preview answers to the window until either side is gone.
async fn follow_answers(
    mut receiver: PreviewReceiver,
    this: gpui::WeakEntity<MainWindow>,
    cx: &mut gpui::AsyncApp,
) {
    while let Some(event) = receiver.next().await {
        let taken = this.update(cx, |this, cx| {
            this.seek_bar.previews.answered(event);
            cx.notify();
        });
        if taken.is_err() {
            break;
        }
    }
}
