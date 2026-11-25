//! Tests of the app's hooks into a measured run, on sessions of their own with a
//! [`FakeProbe`], so they never measure the process's run or race each other.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use gpui::{IntoElement, Render, TestAppContext};

use super::frames::Frame;
use super::probe::{Probe, System};
use super::session::Session;
use super::*;

/// A probe whose clock, CPU time and frames the test sets.
pub(super) struct FakeProbe {
    now: Mutex<Instant>,
    cpu: Mutex<Option<Duration>>,
    frames: Mutex<Vec<(Frame, Instant)>>,
    /// The windows whose frames were asked for, in order.
    asked: Mutex<Vec<u64>>,
}

impl FakeProbe {
    /// A probe whose clock reads `start`, with no CPU used and no frames drawn.
    pub(super) fn new(start: Instant) -> Self {
        Self {
            now: Mutex::new(start),
            cpu: Mutex::new(Some(Duration::ZERO)),
            frames: Mutex::new(Vec::new()),
            asked: Mutex::new(Vec::new()),
        }
    }

    /// Moves the clock on by `by`, the process using `cpu` meanwhile.
    pub(super) fn advance(&self, by: Duration, cpu: Duration) {
        *self.now.lock().unwrap() += by;
        let mut used = self.cpu.lock().unwrap();
        *used = used.map(|used| used + cpu);
    }

    /// The window drew `frame` now; the next poll takes it.
    pub(super) fn draw(&self, frame: Frame) {
        let now = self.now();
        self.frames.lock().unwrap().push((frame, now));
    }

    /// The windows whose frames were asked for, in order.
    pub(super) fn asked(&self) -> Vec<u64> {
        self.asked.lock().unwrap().clone()
    }
}

/// The test keeps the probe to drive it while the session reads it.
impl Probe for &FakeProbe {
    fn now(&self) -> Instant {
        *self.now.lock().unwrap()
    }

    fn cpu_time(&self) -> Option<Duration> {
        *self.cpu.lock().unwrap()
    }

    fn resident_bytes(&self) -> Option<u64> {
        Some(RESIDENT_BYTES)
    }

    fn frames(&self, window: u64) -> Vec<(Frame, Instant)> {
        self.asked.lock().unwrap().push(window);
        std::mem::take(&mut *self.frames.lock().unwrap())
    }
}

/// The memory a [`FakeProbe`] says the process holds: 100 MB.
pub(super) const RESIDENT_BYTES: u64 = 100 << 20;

/// A session of the test's own and its probe, living as long as the app's hooks may.
fn leaked_session() -> (&'static Session<&'static FakeProbe>, &'static FakeProbe) {
    let probe: &'static FakeProbe = Box::leak(Box::new(FakeProbe::new(Instant::now())));
    (Box::leak(Box::new(Session::new(probe))), probe)
}

/// A window with nothing in it.
struct Blank;

impl Render for Blank {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        gpui::Empty
    }
}

/// A path for `name` in the temp dir that no other test or run uses.
fn scratch_file(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("rvlc-perf-test-{}-{name}", std::process::id()))
}

#[test]
fn one_measures_without_a_file_and_a_path_writes_one() {
    assert_eq!(output(OsStr::new(LOG_ONLY)), None);
    let path = "/somewhere/perf.json";
    assert_eq!(output(OsStr::new(path)), Some(PathBuf::from(path)));
}

#[test]
fn the_system_probe_reads_the_process() {
    let before = Instant::now();
    assert!(System.now() >= before);
    if cfg!(unix) {
        assert!(System.cpu_time().is_some());
    }
    if cfg!(any(target_os = "macos", target_os = "linux")) {
        assert!(System.resident_bytes().is_some_and(|bytes| bytes > 0));
    }
}

#[cfg(not(feature = "perf"))]
#[test]
fn without_the_perf_feature_there_are_no_frames() {
    assert!(System.frames(1).is_empty());
}

#[test]
fn the_process_run_is_not_measured_unless_asked() {
    start();
    opening();
    picture_shown();
    phase(Phase::Playing);
    indexed(10, Duration::from_millis(5));
    library_items(10);
    if std::env::var_os(VARIABLE).is_none() {
        assert!(!SESSION.is_measuring());
        assert_eq!(SESSION.rows(), None);
    }
}

#[gpui::test]
fn measuring_polls_until_the_app_quits_and_writes_the_report(cx: &mut TestAppContext) {
    let (session, probe) = leaked_session();
    let report = scratch_file("quit.json");
    let output = Some(report.clone());
    cx.update(|cx| measure(session, Instant::now(), output, cx));
    assert!(session.is_measuring());
    assert_eq!(probe.asked(), [0], "frame timing starts with no window to judge");
    cx.executor().advance_clock(POLL * 2);
    assert_eq!(probe.asked().len(), 3, "a poll each {POLL:?}");
    cx.quit();
    let json = std::fs::read_to_string(&report).expect("the report is written at quit");
    std::fs::remove_file(&report).unwrap();
    let json: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(json["metrics"].as_array().map(Vec::len), Some(7));
}

#[gpui::test]
fn the_main_windows_frames_are_the_ones_judged(cx: &mut TestAppContext) {
    let (session, probe) = leaked_session();
    let (_, cx) = cx.add_window_view(|_, _| Blank);
    cx.update(|window, _| watch_main_window(session, window));
    session.begin(probe.now());
    session.poll();
    assert_eq!(probe.asked(), [0, 0], "a window opened before measuring isn't judged");
    let id = cx.update(|window, _| {
        watch_main_window(session, window);
        window.window_handle().window_id().as_u64()
    });
    session.poll();
    assert_eq!(probe.asked().last(), Some(&id));
}
