//! Measurements: frames shown and dropped, latencies, main-thread paint cost, CPU and memory.

use std::fmt;
use std::time::{Duration, Instant};

use crate::libvlc::MediaStats;
use crate::usage::{peak_rss_mb, process_cpu_time};

/// Samples gathered while the window draws.
pub struct Recorder {
    started: Instant,
    cpu_at_start: Option<Duration>,
    presented: u64,
    due_to_paint_us: Vec<u32>,
    paint_cost_us: Vec<u32>,
    input_to_paint_us: Vec<u32>,
    pending_input: Option<Instant>,
    frame_due: Option<Instant>,
    first_frame: Option<Instant>,
}

impl Recorder {
    /// Starts the clocks.
    pub fn start() -> Self {
        Self {
            started: Instant::now(),
            cpu_at_start: process_cpu_time(),
            presented: 0,
            due_to_paint_us: Vec::new(),
            paint_cost_us: Vec::new(),
            input_to_paint_us: Vec::new(),
            pending_input: None,
            frame_due: None,
            first_frame: None,
        }
    }

    /// A new frame, due at `due`, is about to be painted.
    pub fn new_frame(&mut self, due: Instant) {
        self.presented += 1;
        self.frame_due = Some(due);
        self.first_frame.get_or_insert(due);
    }

    /// The mouse moved at `at`; the next paint answers it.
    pub fn input(&mut self, at: Instant) {
        self.pending_input.get_or_insert(at);
    }

    /// The video element finished painting, having taken `cost` on the main thread.
    pub fn painted(&mut self, cost: Duration) {
        let now = Instant::now();
        self.paint_cost_us.push(micros(cost));
        if let Some(due) = self.frame_due.take() {
            self.due_to_paint_us.push(micros(now - due));
        }
        if let Some(at) = self.pending_input.take() {
            self.input_to_paint_us.push(micros(now - at));
        }
    }

    /// Frames shown per second since the first one (startup is not counted).
    pub fn presented_fps(&self) -> f64 {
        let Some(first) = self.first_frame else { return 0.0 };
        self.presented.saturating_sub(1) as f64 / first.elapsed().as_secs_f64().max(f64::EPSILON)
    }

    /// The summary, given libVLC's counters and the frame slot's `(published, replaced)`.
    pub fn summary(&self, vlc: &MediaStats, slot: (u64, u64)) -> Summary {
        let wall = self.started.elapsed();
        let cpu =
            process_cpu_time().zip(self.cpu_at_start).map(|(now, start)| now.saturating_sub(start));
        let late = u64::try_from(vlc.lost_pictures).unwrap_or(0);
        let offered = u64::try_from(vlc.displayed_pictures).unwrap_or(0) + late;
        Summary {
            seconds: wall.as_secs_f64(),
            presented_fps: self.presented_fps(),
            offered,
            late,
            unseen: slot.1,
            cpu_percent: cpu.map(|cpu| 100.0 * cpu.as_secs_f64() / wall.as_secs_f64()),
            peak_rss_mb: peak_rss_mb(),
            due_to_paint: Spread::of(&self.due_to_paint_us),
            paint_cost: Spread::of(&self.paint_cost_us),
            input_to_paint: Spread::of(&self.input_to_paint_us),
        }
    }
}

fn micros(duration: Duration) -> u32 {
    u32::try_from(duration.as_micros()).unwrap_or(u32::MAX)
}

/// Median, 95th percentile and maximum of a set of microsecond samples, in milliseconds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spread {
    pub p50: f64,
    pub p95: f64,
    pub max: f64,
    pub samples: usize,
}

impl Spread {
    /// The spread of `samples` (microseconds).
    pub fn of(samples: &[u32]) -> Self {
        let mut sorted = samples.to_vec();
        sorted.sort_unstable();
        let ms = |p: f64| f64::from(percentile(&sorted, p)) / 1000.0;
        Self { p50: ms(50.0), p95: ms(95.0), max: ms(100.0), samples: sorted.len() }
    }
}

/// The nearest-rank `p`th percentile of `sorted`, or 0 when it is empty.
pub fn percentile(sorted: &[u32], p: f64) -> u32 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = (p / 100.0 * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

/// The result of one run, printed as one `key=value` line.
pub struct Summary {
    pub seconds: f64,
    pub presented_fps: f64,
    /// Pictures libVLC's clock wanted shown.
    pub offered: u64,
    /// Pictures libVLC dropped as late before reaching us.
    pub late: u64,
    /// Pictures we received but replaced before the window drew them.
    pub unseen: u64,
    pub cpu_percent: Option<f64>,
    pub peak_rss_mb: Option<f64>,
    pub due_to_paint: Spread,
    pub paint_cost: Spread,
    pub input_to_paint: Spread,
}

impl Summary {
    /// Late and unseen pictures as a share of those offered.
    pub fn dropped_percent(&self) -> f64 {
        if self.offered == 0 {
            return 0.0;
        }
        100.0 * (self.late + self.unseen) as f64 / self.offered as f64
    }
}

impl fmt::Display for Summary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let spread =
            |s: Spread| format!("{:.1}/{:.1}/{:.1}ms(n={})", s.p50, s.p95, s.max, s.samples);
        let optional = |v: Option<f64>| v.map_or_else(|| "n/a".into(), |v| format!("{v:.0}"));
        write!(
            f,
            "secs={:.1} fps={:.1} offered={} late={} unseen={} dropped={:.2}% cpu={}% rss={}MB \
             due_to_paint={} paint_cost={} input_to_paint={}",
            self.seconds,
            self.presented_fps,
            self.offered,
            self.late,
            self.unseen,
            self.dropped_percent(),
            optional(self.cpu_percent),
            optional(self.peak_rss_mb),
            spread(self.due_to_paint),
            spread(self.paint_cost),
            spread(self.input_to_paint),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{Spread, percentile};

    #[test]
    fn percentiles_use_the_nearest_rank() {
        let sorted = [10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        assert_eq!(percentile(&sorted, 50.0), 50);
        assert_eq!(percentile(&sorted, 95.0), 100);
        assert_eq!(percentile(&sorted, 100.0), 100);
        assert_eq!(percentile(&sorted, 0.0), 10);
        assert_eq!(percentile(&[], 50.0), 0);
    }

    #[test]
    fn spreads_are_in_milliseconds() {
        let spread = Spread::of(&[3000, 1000, 2000]);
        assert_eq!((spread.p50, spread.max, spread.samples), (2.0, 3.0, 3));
    }
}
