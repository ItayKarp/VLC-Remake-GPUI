//! The seek bar as a subtitle timeline (S6.6): when it shows, what it draws, why the Playback
//! tab's toggle can't turn it on, and its `−` `+` `<` `>` buttons.
//!
//! It shows while Show Subtitle Timeline on Seekbar is on, the media can seek and the times of
//! the subtitle showing are known ([`crate::engine::player::CueSpan`]); otherwise the bar shows
//! the whole media as usual, and turning a subtitle on brings the timeline back.
//!
//! What `*` means (S6.6.2): while the timeline shows, the time readouts show its window's ends,
//! not the media's, and the `*` after them says so. The current time is then in the readouts'
//! tooltips, in the seek bar's spoken value, and in the field that clicking the left readout
//! opens to jump to a time.

use std::time::Duration;

use gpui::{App, Context};

use super::timeline::{Pan, Visible, Zoom};
use crate::engine::player::CueSpan;
use crate::engine::tracks::TrackKind;
use crate::main_window::MainWindow;
use crate::settings;

/// Why the timeline can't show for the media open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::main_window) enum Unavailable {
    /// No subtitle shows.
    NoSubtitle,
    /// The subtitle showing doesn't say when its lines are: the media's own, or pictures.
    UnknownTimes,
}

impl Unavailable {
    /// The line under the toggle saying why it is off.
    pub(in crate::main_window) fn hint(self) -> &'static str {
        match self {
            Self::NoSubtitle => "Turn on a subtitle to see its lines along the seek bar.",
            Self::UnknownTimes => {
                "The times of this subtitle's lines aren't known. Subtitle files VLC reads itself \
                 (SRT, WebVTT, ASS/SSA, MicroDVD) can show; the video's own subtitles and \
                 picture subtitles can't."
            }
        }
    }
}

/// What the timeline draws in one frame.
pub(super) struct Frame {
    /// The stretch of the media it shows.
    pub visible: Visible,
    /// Where playback is.
    pub position: Duration,
    /// The lines in the window, earliest first.
    pub spans: Vec<CueSpan>,
}

impl MainWindow {
    /// The timeline's window this frame, moved along with the playhead; `None` while the bar
    /// shows the whole media. The readouts and the bar ask it alike, so they always agree.
    pub(in crate::main_window) fn subtitle_timeline(&self, cx: &App) -> Option<Visible> {
        let shown = self.follow_timeline(cx);
        self.seek_bar.shown_timeline.set(shown);
        shown
    }

    /// What the timeline draws this frame, while it shows.
    pub(super) fn timeline_frame(&self, cx: &App) -> Option<Frame> {
        let visible = self.subtitle_timeline(cx)?;
        let player = self.loaded_player(cx)?;
        let player = player.read(cx);
        let spans = player.subtitle_spans(visible.start, visible.end())?;
        Some(Frame { visible, position: player.position(), spans })
    }

    /// Why the timeline can't show now, for the toggle's hint; `None` when it can.
    pub(in crate::main_window) fn subtitle_timeline_unavailable(
        &self,
        cx: &App,
    ) -> Option<Unavailable> {
        let player = self.loaded_player(cx)?;
        let player = player.read(cx);
        if player.tracks().selected(TrackKind::Subtitle).is_none() {
            return Some(Unavailable::NoSubtitle);
        }
        (!player.knows_subtitle_timings()).then_some(Unavailable::UnknownTimes)
    }

    /// Which ways zooming and panning would change the window, for its buttons.
    pub(super) fn timeline_moves(&self, cx: &App) -> TimelineMoves {
        let timeline = self.seek_bar.timeline.get();
        let duration = self.seekable(cx).map_or_default(|seekable| seekable.duration);
        TimelineMoves {
            zoom_out: timeline.can_zoom(Zoom::Out, duration),
            zoom_in: timeline.can_zoom(Zoom::In, duration),
            earlier: timeline.can_pan(Pan::Earlier, duration),
            later: timeline.can_pan(Pan::Later, duration),
        }
    }

    /// `−` or `+`: widens or narrows the window.
    pub(super) fn zoom_timeline(&mut self, zoom: Zoom, cx: &mut Context<Self>) {
        let Some(seekable) = self.seekable(cx) else { return };
        let position = seekable.player.read(cx).position();
        self.seek_bar.timeline.get_mut().zoom(zoom, position, seekable.duration);
        cx.notify();
    }

    /// `<` or `>`: moves the window half its width.
    pub(super) fn pan_timeline(&mut self, pan: Pan, cx: &mut Context<Self>) {
        let Some(seekable) = self.seekable(cx) else { return };
        self.seek_bar.timeline.get_mut().pan(pan, seekable.duration);
        cx.notify();
    }

    /// The window for the playhead now, while the timeline can show.
    fn follow_timeline(&self, cx: &App) -> Option<Visible> {
        if !settings::get(cx).prefs.interface.subtitle_timeline {
            return None;
        }
        let seekable = self.seekable(cx)?;
        let player = seekable.player.read(cx);
        if !player.knows_subtitle_timings() {
            return None;
        }
        let mut timeline = self.seek_bar.timeline.get();
        let shown = timeline.follow(player.position(), seekable.duration);
        self.seek_bar.timeline.set(timeline);
        Some(shown)
    }
}

/// Which of the timeline's buttons do something.
#[allow(clippy::struct_excessive_bools, reason = "one flag per button, read once each")]
#[derive(Clone, Copy, Debug)]
pub(super) struct TimelineMoves {
    /// `−`.
    pub zoom_out: bool,
    /// `+`.
    pub zoom_in: bool,
    /// `<`.
    pub earlier: bool,
    /// `>`.
    pub later: bool,
}
