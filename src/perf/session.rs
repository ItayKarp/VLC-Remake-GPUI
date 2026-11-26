//! A measured run, shared by every thread that marks it: the [`Recorder`] behind a lock, the
//! main window it judges, and the [`Probe`] it reads. Until [`Session::begin`] every mark is a
//! no-op that takes no lock.

use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use super::probe::Probe;
use super::recorder::{IndexRun, Phase, Recorder};
use super::report::{self, Row, Usage};

/// The window id asked for when no main window is open yet, so no frames match.
const NO_WINDOW: u64 = 0;

/// A run that is measured once [`Self::begin`] starts it.
pub struct Session<P> {
    probe: P,
    measuring: AtomicBool,
    recorder: Mutex<Option<Recorder>>,
    /// The main window's GPUI id, whose frames are judged; [`NO_WINDOW`] before it opens.
    main_window: AtomicU64,
}

impl<P> Session<P> {
    /// A run that reads `probe` once it is measured.
    pub const fn new(probe: P) -> Self {
        Self {
            probe,
            measuring: AtomicBool::new(false),
            recorder: Mutex::new(None),
            main_window: AtomicU64::new(NO_WINDOW),
        }
    }

    /// Whether the run is measured.
    pub fn is_measuring(&self) -> bool {
        self.measuring.load(Ordering::Acquire)
    }

    fn lock(&self) -> MutexGuard<'_, Option<Recorder>> {
        // A panic while recording leaves nothing half-written that matters: keep measuring.
        self.recorder.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl<P: Probe> Session<P> {
    /// Starts measuring a run whose process started at `start`.
    pub fn begin(&self, start: std::time::Instant) {
        *self.lock() = Some(Recorder::new(start));
        self.measuring.store(true, Ordering::Release);
        // Starts the frame timing, so the first poll has frames to take.
        self.probe.frames(NO_WINDOW);
    }

    /// The main window, with GPUI id `window`, opened: its frames are the ones judged.
    pub fn main_window_opened(&self, window: u64) {
        self.main_window.store(window, Ordering::Release);
    }

    /// The main window drew its first frame.
    pub fn ready(&self) {
        self.record(|run, probe| run.ready(probe.now()));
    }

    /// A file was asked to open: times it until its first picture.
    pub fn opening(&self) {
        self.record(|run, probe| run.opening(probe.now()));
    }

    /// A picture reached the screen.
    pub fn picture_shown(&self) {
        self.record(|run, probe| run.picture(probe.now()));
    }

    /// The player entered `phase`.
    pub fn phase(&self, phase: Phase) {
        self.record(|run, probe| run.phase(phase, probe.now(), probe.cpu_time()));
    }

    /// The library's scan found `files` media in `took`; only the first scan counts.
    pub fn indexed(&self, files: u64, took: Duration) {
        self.record(|run, _| run.indexed(IndexRun { files, took }));
    }

    /// The library holds `items` media.
    pub fn library_items(&self, items: u64) {
        self.record(|run, _| run.library_items(items));
    }

    /// Samples the CPU for a settling pause and takes the frames drawn since the last poll.
    pub fn poll(&self) {
        if !self.is_measuring() {
            return;
        }
        let frames = self.probe.frames(self.main_window.load(Ordering::Acquire));
        self.record(|run, probe| {
            run.tick(probe.now(), probe.cpu_time());
            let before = run.frames().idle_frames();
            for (frame, at) in frames {
                run.frame(frame, at);
            }
            let redrawn = run.frames().idle_frames() - before;
            if redrawn > 0 {
                // Live, so the cause of a redraw loop can be found by what happens meanwhile.
                tracing::info!("perf: the main window redrew {redrawn} times while paused");
            }
        });
    }

    /// The report's rows as of now; `None` while not measuring.
    pub fn rows(&self) -> Option<Vec<Row>> {
        let guard = self.lock();
        let run = guard.as_ref()?;
        let idle = run.idle(self.probe.now(), self.probe.cpu_time());
        Some(report::rows(run, Usage { idle, resident_bytes: self.probe.resident_bytes() }))
    }

    /// Polls a last time, logs the report, and writes it to `output` as JSON when given.
    pub fn report(&self, output: Option<&Path>) {
        self.poll();
        let Some(rows) = self.rows() else { return };
        tracing::info!("{}", report::table(&rows));
        let Some(output) = output else { return };
        if let Err(error) = write_json(&rows, output) {
            tracing::warn!("perf: couldn't write {}: {error}", output.display());
        }
    }

    fn record(&self, change: impl FnOnce(&mut Recorder, &P)) {
        if self.is_measuring()
            && let Some(run) = self.lock().as_mut()
        {
            change(run, &self.probe);
        }
    }
}

/// Writes `rows` to `output` as the report's JSON.
fn write_json(rows: &[Row], output: &Path) -> io::Result<()> {
    std::fs::write(output, serde_json::to_string_pretty(&report::json(rows))?)
}

#[cfg(test)]
mod tests;
