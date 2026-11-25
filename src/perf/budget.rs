//! The performance budgets (S9.3) and how a measurement is judged against them.

use std::time::Duration;

/// A refresh interval at or below this is a 120 Hz display (8.33 ms), with a margin for the
/// intervals displays really report.
const FAST_DISPLAY: Duration = Duration::from_micros(8_500);
/// A frame's budget on a 120 Hz display.
const FAST_FRAME_MS: f64 = 8.;
/// A frame's budget on a 60 Hz display (and anything slower).
const SLOW_FRAME_MS: f64 = 16.;
/// The library size the memory budget is for.
pub const BUDGET_LIBRARY_ITEMS: u64 = 20_000;

/// Something the budgets cover, in the order the PRD lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Metric {
    /// From the process starting to the main window's first frame, in milliseconds.
    ColdStart,
    /// From asking to open a file to its first picture on screen, in milliseconds.
    OpenToFirstFrame,
    /// The 95th percentile of a main-window frame's draw and present while media plays, in
    /// milliseconds.
    PlayingFrameTime,
    /// The process's CPU use while paused, in percent of one core.
    IdleCpu,
    /// Main-window frames drawn per second while paused.
    IdleRedraws,
    /// The process's resident memory with the library open, in megabytes.
    LibraryMemory,
    /// Files indexed per second by the library's first scan.
    IndexRate,
}

impl Metric {
    /// Every metric, in report order.
    pub const ALL: [Self; 7] = [
        Self::ColdStart,
        Self::OpenToFirstFrame,
        Self::PlayingFrameTime,
        Self::IdleCpu,
        Self::IdleRedraws,
        Self::LibraryMemory,
        Self::IndexRate,
    ];

    /// The metric's name in reports, a stable identifier for CI to track.
    pub fn id(self) -> &'static str {
        match self {
            Self::ColdStart => "cold_start_ms",
            Self::OpenToFirstFrame => "open_to_first_frame_ms",
            Self::PlayingFrameTime => "playing_frame_time_p95_ms",
            Self::IdleCpu => "idle_cpu_percent",
            Self::IdleRedraws => "idle_redraws_per_s",
            Self::LibraryMemory => "library_memory_mb",
            Self::IndexRate => "index_files_per_s",
        }
    }

    /// The budget for this metric. The frame time's depends on the display's refresh interval,
    /// when known.
    pub fn budget(self, refresh: Option<Duration>) -> Budget {
        match self {
            Self::ColdStart => Budget::AtMost(1_000.),
            Self::OpenToFirstFrame | Self::LibraryMemory => Budget::AtMost(400.),
            Self::PlayingFrameTime => Budget::AtMost(frame_budget_ms(refresh)),
            Self::IdleCpu => Budget::AtMost(1.),
            Self::IdleRedraws => Budget::AtMost(0.1),
            Self::IndexRate => Budget::AtLeast(200.),
        }
    }
}

/// A frame's budget for a display refreshing every `refresh`: 8 ms at 120 Hz, 16 ms at 60 Hz.
fn frame_budget_ms(refresh: Option<Duration>) -> f64 {
    if refresh.is_some_and(|refresh| refresh <= FAST_DISPLAY) {
        FAST_FRAME_MS
    } else {
        SLOW_FRAME_MS
    }
}

/// The bound a measurement must keep to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Budget {
    /// No more than this.
    AtMost(f64),
    /// No less than this.
    AtLeast(f64),
}

impl Budget {
    /// Whether `value` keeps to the budget.
    pub fn allows(self, value: f64) -> bool {
        match self {
            Self::AtMost(limit) => value <= limit,
            Self::AtLeast(limit) => value >= limit,
        }
    }

    /// The bound as a report writes it: "≤ 400" or "≥ 200".
    pub fn describe(self) -> String {
        match self {
            Self::AtMost(limit) => format!("≤ {limit}"),
            Self::AtLeast(limit) => format!("≥ {limit}"),
        }
    }
}

/// How a metric fared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Measured, within budget.
    Pass,
    /// Measured, over budget.
    Fail,
    /// The run never did what the metric measures.
    NotMeasured,
}

impl Verdict {
    /// Judges `value` against `budget`; no value is [`Verdict::NotMeasured`].
    pub fn of(value: Option<f64>, budget: Budget) -> Self {
        match value {
            Some(value) if budget.allows(value) => Self::Pass,
            Some(_) => Self::Fail,
            None => Self::NotMeasured,
        }
    }

    /// The verdict's name in reports.
    pub fn id(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::NotMeasured => "not_measured",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frame_budget_follows_the_refresh_rate() {
        let at = |hz: u64| Some(Duration::from_nanos(1_000_000_000 / hz));
        assert_eq!(Metric::PlayingFrameTime.budget(at(120)), Budget::AtMost(8.));
        assert_eq!(Metric::PlayingFrameTime.budget(at(60)), Budget::AtMost(16.));
        assert_eq!(Metric::PlayingFrameTime.budget(None), Budget::AtMost(16.));
    }

    #[test]
    fn verdicts_respect_the_bound_direction() {
        let rate = Metric::IndexRate.budget(None);
        assert_eq!(Verdict::of(Some(250.), rate), Verdict::Pass);
        assert_eq!(Verdict::of(Some(150.), rate), Verdict::Fail);
        let start = Metric::ColdStart.budget(None);
        assert_eq!(Verdict::of(Some(1_000.), start), Verdict::Pass);
        assert_eq!(Verdict::of(Some(1_001.), start), Verdict::Fail);
        assert_eq!(Verdict::of(None, start), Verdict::NotMeasured);
    }

    #[test]
    fn metric_ids_are_unique() {
        let mut ids: Vec<_> = Metric::ALL.iter().map(|metric| metric.id()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), Metric::ALL.len());
    }
}
