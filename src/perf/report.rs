//! The budgets report: each metric's measurement and verdict, as a table for the log and JSON
//! for CI to keep.

use std::fmt::Write as _;
use std::time::Duration;

use super::budget::{BUDGET_LIBRARY_ITEMS, Metric, Verdict};
use super::recorder::{Idle, Recorder};

/// Below this much paused time the idle metrics mean little, so they aren't measured.
const MIN_IDLE: Duration = Duration::from_secs(3);
const BYTES_PER_MB: f64 = 1_048_576.;

/// What the process uses at the moment the report is made.
#[derive(Clone, Copy, Debug)]
pub struct Usage {
    /// Paused time and its CPU, up to now.
    pub idle: Idle,
    /// Resident memory now.
    pub resident_bytes: Option<u64>,
}

/// One metric's line in the report.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// What was measured.
    pub metric: Metric,
    /// The measurement, in the metric's unit; `None` when the run didn't measure it.
    pub value: Option<f64>,
    /// How it fared.
    pub verdict: Verdict,
    /// The budget, as written in the report.
    pub budget: String,
    /// What a reader needs to read the value right, if anything.
    pub note: Option<String>,
}

/// Every metric's row for `run`, with the process using `usage`.
pub fn rows(run: &Recorder, usage: Usage) -> Vec<Row> {
    Metric::ALL.iter().map(|&metric| row(metric, run, usage)).collect()
}

fn row(metric: Metric, run: &Recorder, usage: Usage) -> Row {
    let (value, note) = measure(metric, run, usage);
    let budget = metric.budget(run.frames().refresh());
    Row { metric, value, verdict: Verdict::of(value, budget), budget: budget.describe(), note }
}

fn measure(metric: Metric, run: &Recorder, usage: Usage) -> (Option<f64>, Option<String>) {
    let idle = Some(usage.idle).filter(|idle| idle.wall >= MIN_IDLE);
    match metric {
        Metric::ColdStart => (run.cold_start().map(millis), None),
        Metric::OpenToFirstFrame => (run.slowest_open().map(millis), Some("slowest open".into())),
        Metric::PlayingFrameTime => (run.frames().playing_p95_ms(), None),
        Metric::IdleCpu => {
            let percent = idle.and_then(|idle| Some(percent(idle.cpu?, idle.wall)));
            (percent, idle.map(|idle| paused_note(idle.wall)))
        }
        Metric::IdleRedraws => {
            let rate = idle.map(|idle| run.frames().idle_frames() as f64 / idle.wall.as_secs_f64());
            (rate, idle.map(|idle| paused_note(idle.wall)))
        }
        Metric::LibraryMemory => memory(run.items(), usage.resident_bytes),
        Metric::IndexRate => {
            let index = run.index().filter(|index| !index.took.is_zero());
            let rate = index.map(|index| index.files as f64 / index.took.as_secs_f64());
            (rate, index.map(|index| format!("{} files", index.files)))
        }
    }
}

/// Memory is judged only with a library as large as the budget's.
fn memory(items: Option<u64>, resident: Option<u64>) -> (Option<f64>, Option<String>) {
    let megabytes = resident.map(|bytes| bytes as f64 / BYTES_PER_MB);
    let items = items.unwrap_or(0);
    let note = format!("{items} library items");
    if items < BUDGET_LIBRARY_ITEMS {
        let note = format!("{note}, fewer than the budget's {BUDGET_LIBRARY_ITEMS}");
        return (None, Some(megabytes.map_or(note.clone(), |mb| format!("{mb:.0} MB, {note}"))));
    }
    (megabytes, Some(note))
}

fn paused_note(wall: Duration) -> String {
    format!("over {:.0} s paused", wall.as_secs_f64())
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.
}

fn percent(part: Duration, whole: Duration) -> f64 {
    part.as_secs_f64() / whole.as_secs_f64() * 100.
}

/// The rows as an aligned text table, for the log.
pub fn table(rows: &[Row]) -> String {
    let mut text = String::from("performance budgets (S9.3):");
    for row in rows {
        let value = row.value.map_or_else(|| "-".to_owned(), |value| format!("{value:.2}"));
        let note = row.note.as_deref().map_or_else(String::new, |note| format!(" ({note})"));
        let (id, verdict, budget) = (row.metric.id(), row.verdict.id(), &row.budget);
        let _ = write!(text, "\n  {id:<27} {value:>10} {budget:<8} {verdict}{note}");
    }
    text
}

/// The rows as JSON: `{"metrics": [{"id", "value", "budget", "verdict", "note"}, …]}`.
pub fn json(rows: &[Row]) -> serde_json::Value {
    let metrics: Vec<_> = rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "id": row.metric.id(),
                "value": row.value,
                "budget": row.budget,
                "verdict": row.verdict.id(),
                "note": row.note,
            })
        })
        .collect();
    serde_json::json!({ "metrics": metrics })
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::perf::recorder::IndexRun;

    fn usage(idle: Duration, cpu: Duration, resident: u64) -> Usage {
        Usage { idle: Idle { wall: idle, cpu: Some(cpu) }, resident_bytes: Some(resident) }
    }

    fn value(rows: &[Row], metric: Metric) -> &Row {
        rows.iter().find(|row| row.metric == metric).expect("every metric has a row")
    }

    #[test]
    fn an_empty_run_measures_nothing_but_memory_in_a_note() {
        let rows = rows(&Recorder::new(Instant::now()), usage(Duration::ZERO, Duration::ZERO, 0));
        assert!(rows.iter().all(|row| row.verdict == Verdict::NotMeasured));
        let note = value(&rows, Metric::LibraryMemory).note.clone().unwrap_or_default();
        assert!(note.contains("0 library items"), "{note}");
    }

    #[test]
    fn idle_cpu_and_index_rate_are_rates() {
        let mut run = Recorder::new(Instant::now());
        run.indexed(IndexRun { files: 1_000, took: Duration::from_secs(2) });
        run.library_items(25_000);
        let used = usage(Duration::from_secs(10), Duration::from_millis(50), 300 << 20);
        let rows = rows(&run, used);
        assert_eq!(value(&rows, Metric::IndexRate).value, Some(500.));
        assert_eq!(value(&rows, Metric::IdleCpu).value, Some(0.5));
        assert_eq!(value(&rows, Metric::IdleRedraws).value, Some(0.));
        assert_eq!(value(&rows, Metric::LibraryMemory).value, Some(300.));
        assert_eq!(value(&rows, Metric::LibraryMemory).verdict, Verdict::Pass);
    }

    #[test]
    fn the_json_names_every_metric() {
        let rows = rows(&Recorder::new(Instant::now()), usage(Duration::ZERO, Duration::ZERO, 0));
        let json = json(&rows);
        assert_eq!(json["metrics"].as_array().map(Vec::len), Some(Metric::ALL.len()));
        assert_eq!(json["metrics"][0]["id"], "cold_start_ms");
        assert!(table(&rows).contains("index_files_per_s"));
    }
}
