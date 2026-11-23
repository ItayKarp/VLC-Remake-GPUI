//! The subtitle timeline's window (S6.6.1, S6.6.2): which stretch of the media the seek bar
//! shows while it is a timeline, kept apart from drawing so it can be tested.
//!
//! The window is 50 s wide at first, as in `controls-sub_timeline.png`; `−` and `+` step through
//! [`SPANS`], never wider than the media. It holds still while the playhead crosses it, so the
//! lines can be read, and moves on a page once the playhead leaves it, putting the playhead in
//! its middle. `<` and `>` move it half its width and pin it there, away from the playhead, until
//! the playhead is inside it again or the user seeks from the bar.

use std::time::Duration;

/// The widths the window zooms through, narrowest first.
pub const SPANS: [Duration; 7] = [
    Duration::from_secs(10),
    Duration::from_secs(20),
    Duration::from_secs(30),
    Duration::from_secs(50),
    Duration::from_secs(120),
    Duration::from_secs(300),
    Duration::from_secs(600),
];
/// The designed width: 50 s.
const DEFAULT_ZOOM: usize = 3;

/// Which way `−` and `+` zoom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zoom {
    /// `−`: a wider window.
    Out,
    /// `+`: a narrower one.
    In,
}

/// Which way `<` and `>` pan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pan {
    /// `<`: earlier in the media.
    Earlier,
    /// `>`: later.
    Later,
}

/// The stretch of the media the timeline shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Visible {
    /// Its first moment.
    pub start: Duration,
    /// How long it is.
    pub span: Duration,
}

impl Visible {
    /// Its last moment.
    pub fn end(self) -> Duration {
        self.start + self.span
    }

    /// The time at `fraction` of the way across it.
    pub fn time_at(self, fraction: f32) -> Duration {
        self.start + self.span.mul_f64(f64::from(fraction.clamp(0., 1.)))
    }

    /// How far across it `time` is, clamped to 0–1; 0 for an empty window.
    pub fn fraction_of(self, time: Duration) -> f32 {
        if self.span.is_zero() {
            return 0.;
        }
        let into = time.saturating_sub(self.start).as_secs_f64();
        (into / self.span.as_secs_f64()).clamp(0., 1.) as f32
    }

    /// Whether `time` is inside it, either end included.
    pub fn covers(self, time: Duration) -> bool {
        (self.start..=self.end()).contains(&time)
    }
}

/// Where the window is and how wide, and whether the user moved it away from the playhead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timeline {
    start: Duration,
    zoom: usize,
    pinned: bool,
}

impl Default for Timeline {
    fn default() -> Self {
        Self { start: Duration::ZERO, zoom: DEFAULT_ZOOM, pinned: false }
    }
}

impl Timeline {
    /// The window for the playhead at `position` of media `duration` long: where it was, unless
    /// the playhead left it (and the user didn't move it), then a page on with the playhead in
    /// its middle.
    pub fn follow(&mut self, position: Duration, duration: Duration) -> Visible {
        let mut shown = self.at(self.start, duration);
        if shown.covers(position) {
            self.pinned = false;
        } else if !self.pinned {
            shown = self.at(position.saturating_sub(shown.span / 2), duration);
        }
        self.start = shown.start;
        shown
    }

    /// Moves the window half its width `pan`, away from the playhead.
    pub fn pan(&mut self, pan: Pan, duration: Duration) {
        let step = self.span(duration) / 2;
        let start = match pan {
            Pan::Earlier => self.start.saturating_sub(step),
            Pan::Later => self.start + step,
        };
        self.start = self.at(start, duration).start;
        self.pinned = true;
    }

    /// Widens or narrows the window around the playhead at `position` when it shows it, else
    /// around its middle.
    pub fn zoom(&mut self, zoom: Zoom, position: Duration, duration: Duration) {
        if !self.can_zoom(zoom, duration) {
            return;
        }
        let before = self.at(self.start, duration);
        let centre =
            if before.covers(position) { position } else { before.start + before.span / 2 };
        self.zoom = match zoom {
            Zoom::Out => self.zoom + 1,
            Zoom::In => self.zoom - 1,
        };
        let span = self.span(duration);
        self.start = self.at(centre.saturating_sub(span / 2), duration).start;
    }

    /// Whether zooming `zoom` changes anything: in, until the narrowest; out, until the widest
    /// or the whole media shows.
    pub fn can_zoom(self, zoom: Zoom, duration: Duration) -> bool {
        match zoom {
            Zoom::In => self.zoom > 0 && SPANS[self.zoom - 1] < duration,
            Zoom::Out => self.zoom + 1 < SPANS.len() && SPANS[self.zoom] < duration,
        }
    }

    /// Whether panning `pan` moves the window: it isn't at that end of the media.
    pub fn can_pan(self, pan: Pan, duration: Duration) -> bool {
        let shown = self.at(self.start, duration);
        match pan {
            Pan::Earlier => !shown.start.is_zero(),
            Pan::Later => shown.end() < duration,
        }
    }

    /// Lets the window follow the playhead again: the user seeked from the bar.
    pub fn unpin(&mut self) {
        self.pinned = false;
    }

    /// Back to the start of new media, at the same zoom.
    pub fn restart(&mut self) {
        *self = Self { zoom: self.zoom, ..Self::default() };
    }

    /// The window from `start`, kept inside the media.
    fn at(self, start: Duration, duration: Duration) -> Visible {
        let span = self.span(duration);
        Visible { start: start.min(duration.saturating_sub(span)), span }
    }

    /// The window's width: the zoom's, or the whole media when that is shorter.
    fn span(self, duration: Duration) -> Duration {
        SPANS[self.zoom].min(duration)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: Duration = Duration::from_secs(3_600);

    fn secs(seconds: u64) -> Duration {
        Duration::from_secs(seconds)
    }

    #[test]
    fn the_window_holds_still_then_pages_to_the_playhead() {
        let mut timeline = Timeline::default();
        assert_eq!(timeline.follow(secs(10), HOUR), Visible { start: secs(0), span: secs(50) });
        assert_eq!(timeline.follow(secs(49), HOUR).start, secs(0), "it holds still");
        assert_eq!(timeline.follow(secs(51), HOUR).start, secs(26), "the playhead in the middle");
        assert_eq!(timeline.follow(HOUR, HOUR).end(), HOUR, "it stays inside the media");
    }

    #[test]
    fn panning_pins_it_until_the_playhead_is_back_inside() {
        let mut timeline = Timeline::default();
        timeline.follow(secs(10), HOUR);
        timeline.pan(Pan::Later, HOUR);
        timeline.pan(Pan::Later, HOUR);
        assert_eq!(timeline.follow(secs(11), HOUR).start, secs(50), "pinned away");
        assert_eq!(timeline.follow(secs(60), HOUR).start, secs(50), "the playhead is back in");
        assert_eq!(timeline.follow(secs(101), HOUR).start, secs(76), "so it follows again");
        timeline.pan(Pan::Earlier, HOUR);
        timeline.pan(Pan::Earlier, HOUR);
        timeline.pan(Pan::Earlier, HOUR);
        timeline.pan(Pan::Earlier, HOUR);
        assert_eq!(timeline.follow(secs(101), HOUR).start, secs(0));
        assert!(!timeline.can_pan(Pan::Earlier, HOUR));
    }

    #[test]
    fn zooming_keeps_the_playhead_in_the_middle() {
        let mut timeline = Timeline::default();
        timeline.follow(secs(100), HOUR);
        timeline.zoom(Zoom::In, secs(100), HOUR);
        assert_eq!(timeline.follow(secs(100), HOUR), Visible { start: secs(85), span: secs(30) });
        timeline.zoom(Zoom::Out, secs(100), HOUR);
        timeline.zoom(Zoom::Out, secs(100), HOUR);
        assert_eq!(timeline.follow(secs(100), HOUR), Visible { start: secs(40), span: secs(120) });
    }

    #[test]
    fn short_media_shows_whole_and_cannot_zoom_out() {
        let mut timeline = Timeline::default();
        let short = secs(40);
        assert_eq!(timeline.follow(secs(5), short), Visible { start: secs(0), span: short });
        assert!(!timeline.can_zoom(Zoom::Out, short));
        assert!(timeline.can_zoom(Zoom::In, short));
        assert!(!timeline.can_pan(Pan::Later, short));
        assert!(!Timeline::default().can_zoom(Zoom::In, secs(15)), "20 s would show it all too");
    }

    #[test]
    fn fractions_map_across_the_window() {
        let shown = Visible { start: secs(100), span: secs(50) };
        assert_eq!(shown.time_at(0.5), secs(125));
        assert!((shown.fraction_of(secs(110)) - 0.2).abs() < 1e-6);
        assert!(shown.fraction_of(secs(10)).abs() < f32::EPSILON);
        let empty = Visible { start: secs(0), span: Duration::ZERO };
        assert!(empty.fraction_of(secs(1)).abs() < f32::EPSILON);
    }
}
