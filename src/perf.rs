//! Performance budgets (S9.3): measures a run against the budgets the PRD sets, when asked to.
//!
//! `RVLC_PERF=1` measures and logs the report when the app quits; `RVLC_PERF=<file>` writes it
//! there as JSON too, for CI to keep. Without it every mark here is a no-op. Frame times and
//! idle redraws need the `perf` cargo feature, which turns on GPUI's frame timing; the rest is
//! measured in any build.
//!
//! - [`budget`]: the budgets and verdicts.
//! - [`recorder`], [`frames`]: what the run saw.
//! - [`process`]: the CPU and memory the OS says the process uses; [`probe`] reads them.
//! - [`session`]: the run being measured, which every mark here goes to.
//! - [`report`]: the table and JSON made from them.

pub mod budget;
#[cfg(feature = "perf")]
mod collector;
#[cfg_attr(
    not(feature = "perf"),
    allow(dead_code, reason = "frames come from GPUI's frame timing, built by the perf feature")
)]
pub mod frames;
pub mod probe;
pub mod process;
pub mod recorder;
pub mod report;
pub mod session;

use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use gpui::{App, Window};

use self::probe::{Probe, System};
use self::session::Session;

pub use self::recorder::Phase;

/// The variable that turns measuring on: `1`, or the file to write the JSON report to.
const VARIABLE: &str = "RVLC_PERF";
/// The variable's value that measures without writing a file.
const LOG_ONLY: &str = "1";
/// How often the run samples the CPU and collects frames.
const POLL: Duration = Duration::from_millis(250);

static START: OnceLock<Instant> = OnceLock::new();
/// This process's run.
static SESSION: Session<System> = Session::new(System);

/// Notes when the process started. Call first thing in `main`, before anything slow.
pub fn start() {
    START.get_or_init(Instant::now);
}

/// Starts measuring when `RVLC_PERF` asks to: samples the process as it runs and reports when
/// the app quits.
pub fn init(cx: &mut App) {
    let Some(value) = std::env::var_os(VARIABLE) else { return };
    let start = *START.get_or_init(Instant::now);
    measure(&SESSION, start, output(&value), cx);
}

/// Where `RVLC_PERF`'s `value` asks the JSON report to go; `None` to only log it.
fn output(value: &OsStr) -> Option<PathBuf> {
    Some(PathBuf::from(value)).filter(|path| path.as_os_str() != LOG_ONLY)
}

/// Measures `session`, which started at `start`: polls it every [`POLL`] and reports it,
/// to `output` too when given, when the app quits.
fn measure<P: Probe + 'static>(
    session: &'static Session<P>,
    start: Instant,
    output: Option<PathBuf>,
    cx: &mut App,
) {
    session.begin(start);
    tracing::info!("perf: measuring this run against the budgets (S9.3)");
    cx.on_app_quit(move |_| {
        session.report(output.as_deref());
        async {}
    })
    .detach();
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(POLL).await;
            session.poll();
        }
    })
    .detach();
}

/// The main window opened in `window`: its frames are the ones judged, and its first frame
/// ends the cold start.
pub fn main_window_opened(window: &mut Window) {
    watch_main_window(&SESSION, window);
}

/// [`main_window_opened`] for `session`.
fn watch_main_window<P: Probe + 'static>(session: &'static Session<P>, window: &mut Window) {
    if !session.is_measuring() {
        return;
    }
    session.main_window_opened(window.window_handle().window_id().as_u64());
    // The first callback runs before the first frame is drawn; the one it asks for, after.
    window.on_next_frame(|window, _| {
        window.on_next_frame(|_, _| session.ready());
    });
}

/// A file was asked to open: times it until its first picture.
pub fn opening() {
    SESSION.opening();
}

/// A picture reached the screen.
pub fn picture_shown() {
    SESSION.picture_shown();
}

/// The player entered `phase`.
pub fn phase(phase: Phase) {
    SESSION.phase(phase);
}

/// The library's scan found `files` media in `took`; only the first scan counts.
pub fn indexed(files: u64, took: Duration) {
    SESSION.indexed(files, took);
}

/// The library holds `items` media.
pub fn library_items(items: u64) {
    SESSION.library_items(items);
}

#[cfg(test)]
mod tests;
