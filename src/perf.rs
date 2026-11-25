//! Performance budgets (S9.3): measures a run against the budgets the PRD sets, when asked to.
//!
//! `RVLC_PERF=1` measures and logs the report when the app quits; `RVLC_PERF=<file>` writes it
//! there as JSON too, for CI to keep. Without it every mark here is a no-op. Frame times and
//! idle redraws need the `perf` cargo feature, which turns on GPUI's frame timing; the rest is
//! measured in any build.
//!
//! - [`budget`]: the budgets and verdicts.
//! - [`recorder`], [`frames`]: what the run saw.
//! - [`process`]: the CPU and memory the OS says the process uses.
//! - [`report`]: the table and JSON made from them.

pub mod budget;
#[cfg(feature = "perf")]
mod collector;
#[cfg_attr(
    not(feature = "perf"),
    allow(dead_code, reason = "frames come from GPUI's frame timing, built by the perf feature")
)]
pub mod frames;
pub mod process;
pub mod recorder;
pub mod report;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use gpui::{App, Window};

use self::recorder::{IndexRun, Recorder};
use self::report::Usage;

pub use self::recorder::Phase;

/// The variable that turns measuring on: `1`, or the file to write the JSON report to.
const VARIABLE: &str = "RVLC_PERF";
/// How often the run samples the CPU and collects frames.
const POLL: Duration = Duration::from_millis(250);

static START: OnceLock<Instant> = OnceLock::new();
static ENABLED: AtomicBool = AtomicBool::new(false);
static RECORDER: Mutex<Option<Recorder>> = Mutex::new(None);
/// The main window's GPUI id, whose frames are judged; 0 before it opens.
static MAIN_WINDOW: AtomicU64 = AtomicU64::new(0);

/// Notes when the process started. Call first thing in `main`, before anything slow.
pub fn start() {
    START.get_or_init(Instant::now);
}

/// Starts measuring when `RVLC_PERF` asks to: samples the process as it runs and reports when
/// the app quits.
pub fn init(cx: &mut App) {
    let Some(target) = std::env::var_os(VARIABLE) else { return };
    let start = *START.get_or_init(Instant::now);
    *lock() = Some(Recorder::new(start));
    ENABLED.store(true, Ordering::Release);
    #[cfg(feature = "perf")]
    collector::take(0);
    tracing::info!("perf: measuring this run against the budgets (S9.3)");
    let output = Some(PathBuf::from(target)).filter(|path| path.as_os_str() != "1");
    cx.on_app_quit(move |_| {
        report(output.as_deref());
        async {}
    })
    .detach();
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(POLL).await;
            poll();
        }
    })
    .detach();
}

/// The main window opened in `window`: its frames are the ones judged, and its first frame
/// ends the cold start.
pub fn main_window_opened(window: &mut Window) {
    if !enabled() {
        return;
    }
    MAIN_WINDOW.store(window.window_handle().window_id().as_u64(), Ordering::Release);
    // The first callback runs before the first frame is drawn; the one it asks for, after.
    window.on_next_frame(|window, _| {
        window.on_next_frame(|_, _| record(|run| run.ready(Instant::now())));
    });
}

/// A file was asked to open: times it until its first picture.
pub fn opening() {
    record(|run| run.opening(Instant::now()));
}

/// A picture reached the screen.
pub fn picture_shown() {
    record(|run| run.picture(Instant::now()));
}

/// The player entered `phase`.
pub fn phase(phase: Phase) {
    record(|run| run.phase(phase, Instant::now(), process::cpu_time()));
}

/// The library's scan found `files` media in `took`; only the first scan counts.
pub fn indexed(files: u64, took: Duration) {
    record(|run| run.indexed(IndexRun { files, took }));
}

/// The library holds `items` media.
pub fn library_items(items: u64) {
    record(|run| run.library_items(items));
}

fn enabled() -> bool {
    ENABLED.load(Ordering::Acquire)
}

fn record(change: impl FnOnce(&mut Recorder)) {
    if enabled()
        && let Some(run) = lock().as_mut()
    {
        change(run);
    }
}

fn lock() -> std::sync::MutexGuard<'static, Option<Recorder>> {
    // A panic while recording leaves nothing half-written that matters: keep measuring.
    RECORDER.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Samples the CPU for a settling pause and takes the frames drawn since the last poll.
fn poll() {
    #[cfg(feature = "perf")]
    let frames = collector::take(MAIN_WINDOW.load(Ordering::Acquire));
    record(|run| {
        run.tick(Instant::now(), process::cpu_time());
        #[cfg(feature = "perf")]
        {
            let before = run.frames().idle_frames();
            for (frame, at) in frames {
                run.frame(frame, at);
            }
            let redrawn = run.frames().idle_frames() - before;
            if redrawn > 0 {
                // Live, so the cause of a redraw loop can be found by what happens meanwhile.
                tracing::info!("perf: the main window redrew {redrawn} times while paused");
            }
        }
    });
}

/// Logs the report, and writes it to `output` as JSON when given.
fn report(output: Option<&Path>) {
    poll();
    let Some(rows) = rows() else { return };
    tracing::info!("{}", report::table(&rows));
    if let Some(output) = output {
        write_json(&rows, output);
    }
}

/// The report's rows as of now.
fn rows() -> Option<Vec<report::Row>> {
    let guard = lock();
    let run = guard.as_ref()?;
    let idle = run.idle(Instant::now(), process::cpu_time());
    Some(report::rows(run, Usage { idle, resident_bytes: process::resident_bytes() }))
}

fn write_json(rows: &[report::Row], output: &Path) {
    let json = serde_json::to_string_pretty(&report::json(rows)).unwrap_or_default();
    if let Err(error) = std::fs::write(output, json) {
        tracing::warn!("perf: couldn't write {}: {error}", output.display());
    }
}
