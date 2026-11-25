use std::path::PathBuf;
use std::time::Instant;

use super::*;
use crate::perf::budget::{Metric, Verdict};
use crate::perf::frames::Frame;
use crate::perf::recorder::SETTLE;
use crate::perf::tests::{FakeProbe, RESIDENT_BYTES};

fn ms(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

/// A frame that took `draw` to draw and nothing to present.
fn frame(draw: Duration) -> Frame {
    Frame { draw, present: Duration::ZERO, refresh: None }
}

/// The value `rows` hold for `metric`.
fn value(rows: &[Row], metric: Metric) -> Option<f64> {
    rows.iter().find(|row| row.metric == metric).and_then(|row| row.value)
}

/// A session reading `probe`, measuring since the probe's clock reads now.
fn measured(probe: &FakeProbe) -> Session<&FakeProbe> {
    let session = Session::new(probe);
    session.begin(probe.now());
    session
}

/// A path for `name` in the temp dir that no other test or run uses.
fn scratch_file(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("rvlc-perf-session-test-{}-{name}", std::process::id()))
}

#[test]
fn nothing_is_measured_before_the_run_begins() {
    let probe = FakeProbe::new(Instant::now());
    let session = Session::new(&probe);
    session.opening();
    session.picture_shown();
    session.phase(Phase::Playing);
    session.poll();
    assert!(!session.is_measuring());
    assert!(probe.asked().is_empty(), "the frame timing isn't started");
    assert_eq!(session.rows(), None);
    let report = scratch_file("unmeasured.json");
    session.report(Some(&report));
    assert!(!report.exists(), "there is no report to write");
}

#[test]
fn beginning_starts_the_frame_timing_and_polls_ask_for_the_main_windows_frames() {
    let probe = FakeProbe::new(Instant::now());
    let session = measured(&probe);
    session.poll();
    session.main_window_opened(7);
    session.poll();
    assert_eq!(probe.asked(), [0, 0, 7]);
}

#[test]
fn the_cold_start_and_each_open_are_timed_by_the_probes_clock() {
    let probe = FakeProbe::new(Instant::now());
    let session = measured(&probe);
    probe.advance(ms(250), Duration::ZERO);
    session.ready();
    session.opening();
    probe.advance(ms(80), Duration::ZERO);
    session.picture_shown();
    let rows = session.rows().unwrap();
    assert_eq!(value(&rows, Metric::ColdStart), Some(250.));
    assert_eq!(value(&rows, Metric::OpenToFirstFrame), Some(80.));
}

#[test]
fn frames_and_cpu_while_paused_count_once_the_pause_settles() {
    let probe = FakeProbe::new(Instant::now());
    let session = measured(&probe);
    session.phase(Phase::Playing);
    probe.advance(ms(10), Duration::ZERO);
    probe.draw(frame(ms(4)));
    probe.advance(ms(10), Duration::ZERO);
    session.phase(Phase::Paused);
    probe.advance(SETTLE, ms(30));
    session.poll();
    probe.advance(Duration::from_secs(4), ms(40));
    probe.draw(frame(ms(2)));
    probe.draw(frame(ms(2)));
    session.poll();
    let rows = session.rows().unwrap();
    assert_eq!(value(&rows, Metric::PlayingFrameTime), Some(4.));
    assert_eq!(value(&rows, Metric::IdleCpu), Some(1.), "40 ms of CPU in 4 s");
    assert_eq!(value(&rows, Metric::IdleRedraws), Some(0.5));
}

#[test]
fn the_library_is_judged_by_its_first_scan_and_its_size() {
    let probe = FakeProbe::new(Instant::now());
    let session = measured(&probe);
    session.indexed(5_000, Duration::from_secs(2));
    session.indexed(10, ms(1));
    session.library_items(20_000);
    let rows = session.rows().unwrap();
    assert_eq!(value(&rows, Metric::IndexRate), Some(2_500.));
    let megabytes = RESIDENT_BYTES as f64 / 1_048_576.;
    assert_eq!(value(&rows, Metric::LibraryMemory), Some(megabytes));
}

#[test]
fn the_report_is_written_as_json_with_a_last_poll() {
    let probe = FakeProbe::new(Instant::now());
    let session = measured(&probe);
    probe.advance(ms(200), Duration::ZERO);
    session.ready();
    let report = scratch_file("report.json");
    session.report(Some(&report));
    assert_eq!(probe.asked().len(), 2, "the report polls first");
    let json = std::fs::read_to_string(&report).unwrap();
    std::fs::remove_file(&report).unwrap();
    let json: serde_json::Value = serde_json::from_str(&json).unwrap();
    let cold_start = &json["metrics"][0];
    assert_eq!(cold_start["id"], Metric::ColdStart.id());
    assert_eq!(cold_start["value"], 200.);
    assert_eq!(cold_start["verdict"], Verdict::Pass.id());
}

#[test]
fn a_report_that_cant_be_written_is_only_logged() {
    let probe = FakeProbe::new(Instant::now());
    let session = measured(&probe);
    let report = scratch_file("missing-folder").join("report.json");
    session.report(Some(&report));
    assert!(!report.exists());
    session.report(None);
}
