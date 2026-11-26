//! The main window's frames, tallied by what the player was doing while they were drawn.

use std::time::Duration;

/// Which quantile of the playing frames' times the budget judges.
const QUANTILE: f64 = 0.95;

/// One frame the main window drew and presented.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    /// Drawing it, from layout to paint.
    pub draw: Duration,
    /// Handing it to the OS, when known.
    pub present: Duration,
    /// The display's refresh interval, when the OS reports it.
    pub refresh: Option<Duration>,
}

impl Frame {
    /// The frame's cost on the UI thread.
    fn cost(self) -> Duration {
        self.draw + self.present
    }
}

/// The frames drawn so far.
#[derive(Debug, Default)]
pub struct FrameTally {
    /// The cost of each frame drawn while media played, in milliseconds.
    playing_ms: Vec<f64>,
    /// The last refresh interval reported while playing.
    refresh: Option<Duration>,
    /// Frames drawn while paused, after the pause settled.
    idle: u64,
}

impl FrameTally {
    /// Counts a frame drawn while media played.
    pub fn playing(&mut self, frame: Frame) {
        self.playing_ms.push(frame.cost().as_secs_f64() * 1_000.);
        self.refresh = frame.refresh.or(self.refresh);
    }

    /// Counts a frame drawn while paused, with nothing meant to change.
    pub fn idle(&mut self) {
        self.idle += 1;
    }

    /// Frames drawn while paused.
    pub fn idle_frames(&self) -> u64 {
        self.idle
    }

    /// The display's refresh interval while playing, when known.
    pub fn refresh(&self) -> Option<Duration> {
        self.refresh
    }

    /// The 95th percentile of the playing frames' cost, in milliseconds; `None` before any.
    pub fn playing_p95_ms(&self) -> Option<f64> {
        quantile(&self.playing_ms, QUANTILE)
    }
}

/// The `q` quantile of `values` (nearest rank), or `None` for no values.
fn quantile(values: &[f64], q: f64) -> Option<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let rank = (q * sorted.len() as f64).ceil() as usize;
    sorted.get(rank.saturating_sub(1)).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ms: u64) -> Frame {
        let refresh = Some(Duration::from_micros(16_667));
        Frame { draw: Duration::from_millis(ms), present: Duration::ZERO, refresh }
    }

    #[test]
    fn the_95th_percentile_ignores_the_slowest_twentieth() {
        let mut tally = FrameTally::default();
        for ms in 1..=20 {
            tally.playing(frame(ms));
        }
        assert_eq!(tally.playing_p95_ms(), Some(19.));
        assert_eq!(tally.refresh(), Some(Duration::from_micros(16_667)));
    }

    #[test]
    fn no_frames_measure_nothing() {
        assert_eq!(FrameTally::default().playing_p95_ms(), None);
    }

    #[test]
    fn a_frame_costs_its_draw_and_present() {
        let mut tally = FrameTally::default();
        let present = Duration::from_millis(2);
        tally.playing(Frame { present, ..frame(3) });
        assert_eq!(tally.playing_p95_ms(), Some(5.));
    }
}
