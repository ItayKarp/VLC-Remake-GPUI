//! Pressing a subtitle line on the timeline (S6.6.3): a click seeks to where the line starts, and
//! a drag moves every line of that subtitle along with the pointer, changing its delay as G and
//! H do. The delay shows over the video as it changes, and is remembered for the media once the
//! line is let go.

use std::time::Duration;

use gpui::{Context, Pixels};

use super::Seekable;
use super::geometry::Span;
use super::timeline::Visible;
use crate::engine::player::CueSpan;
use crate::engine::values::{Offset, SeekTarget};
use crate::main_window::MainWindow;

/// How far the pointer moves before a press on a line is a drag rather than a click.
const DRAG_THRESHOLD: f32 = 3.;
/// The narrowest a line's block is drawn, so a short line can be seen and pressed.
pub(super) const MIN_BLOCK: f32 = 3.;
/// Milliseconds in a second.
const MS_PER_SECOND: f64 = 1_000.;

/// A subtitle line pressed on the timeline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct CueDrag {
    /// Where the pointer pressed it.
    anchor_x: f32,
    /// The subtitle's delay then.
    delay: Offset,
    /// When the line starts, where a click seeks.
    start: Duration,
    /// The timeline's width in time, which a pixel of drag is a share of.
    span: Duration,
    /// Whether it moved far enough to be a drag.
    moved: bool,
}

impl CueDrag {
    /// The delay with the pointer at `x` over a track `width` wide; `None` until the pointer
    /// moved far enough to make it a drag.
    fn delay_at(&mut self, x: f32, width: f32) -> Option<Offset> {
        let moved = x - self.anchor_x;
        if !self.moved && moved.abs() < DRAG_THRESHOLD {
            return None;
        }
        self.moved = true;
        let seconds = f64::from(moved) / f64::from(width.max(1.)) * self.span.as_secs_f64();
        let millis = (seconds * MS_PER_SECOND).round() as i64;
        Some(Offset::from_millis(self.delay.as_millis() + millis))
    }
}

/// The line drawn under window position `x` on the timeline `shown` across `track`; of lines
/// drawn over each other, the one on top.
fn line_at(x: f32, track: Span, shown: Visible, spans: &[CueSpan]) -> Option<CueSpan> {
    spans.iter().rev().copied().find(|span| {
        let left = track.x_of(shown.fraction_of(span.start));
        let right = track.x_of(shown.fraction_of(span.end)).max(left + MIN_BLOCK);
        (left..=right).contains(&x)
    })
}

impl MainWindow {
    /// Presses the line under `x` on the timeline across `track`, if there is one there.
    pub(super) fn press_cue(&mut self, x: Pixels, track: Span, cx: &mut Context<Self>) -> bool {
        let x = f32::from(x);
        let Some(frame) = self.timeline_frame(cx) else { return false };
        let Some(line) = line_at(x, track, frame.visible, &frame.spans) else { return false };
        let Some(player) = self.loaded_player(cx) else { return false };
        let delay = player.read(cx).subtitle_sync().delay();
        let span = frame.visible.span;
        self.seek_bar.cue_drag =
            Some(CueDrag { anchor_x: x, delay, start: line.start, span, moved: false });
        cx.notify();
        true
    }

    /// Moves the pressed line to the pointer at `x`; `false` while no line is pressed.
    pub(super) fn drag_cue(&mut self, x: Pixels, track: Span, cx: &mut Context<Self>) -> bool {
        let Some(drag) = &mut self.seek_bar.cue_drag else { return false };
        if let Some(delay) = drag.delay_at(f32::from(x), track.width) {
            self.drag_subtitle_delay(delay, cx);
        }
        true
    }

    /// Lets the pressed line go: a drag's delay is remembered, and a click seeks to the line's
    /// start. `false` while no line is pressed.
    pub(super) fn release_cue(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(drag) = self.seek_bar.cue_drag.take() else { return false };
        if drag.moved {
            self.remember_subtitle_sync(cx);
        } else if let Some(Seekable { player, .. }) = self.seekable(cx) {
            self.seek_bar.timeline.get_mut().unpin();
            player.update(cx, |player, cx| player.seek(SeekTarget::Absolute(drag.start), cx));
        }
        cx.notify();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRACK: Span = Span { left: 100., width: 1000. };
    const SHOWN: Visible =
        Visible { start: Duration::from_secs(100), span: Duration::from_secs(50) };

    fn line(start_ms: u64, end_ms: u64) -> CueSpan {
        CueSpan { start: Duration::from_millis(start_ms), end: Duration::from_millis(end_ms) }
    }

    #[test]
    fn a_press_finds_the_line_under_it() {
        let spans = [line(110_000, 115_000), line(120_000, 120_010)];
        assert_eq!(line_at(350., TRACK, SHOWN, &spans), Some(spans[0]));
        assert_eq!(line_at(495., TRACK, SHOWN, &spans), None);
        assert_eq!(line_at(502., TRACK, SHOWN, &spans), Some(spans[1]), "short lines are wider");
    }

    #[test]
    fn dragging_turns_pixels_into_delay_past_a_threshold() {
        let mut drag = CueDrag {
            anchor_x: 500.,
            delay: Offset::from_millis(-100),
            start: Duration::ZERO,
            span: Duration::from_secs(50),
            moved: false,
        };
        assert_eq!(drag.delay_at(502., 1000.), None, "a click wobbles");
        assert_eq!(drag.delay_at(520., 1000.), Some(Offset::from_millis(900)));
        assert_eq!(drag.delay_at(501., 1000.), Some(Offset::from_millis(-50)), "now it's a drag");
    }
}
