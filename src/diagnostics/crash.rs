//! Crash reports (S0.7.2). A panic anywhere writes `crash-<time>.txt` to the logs
//! folder (backtrace, version, OS, GPU) and ends the app; the next launch offers to
//! show the report. Reports are never uploaded.
//!
//! Every panic ends the process, background threads included: a dead worker could
//! otherwise leave the UI waiting forever on work that will never finish.

use std::backtrace::Backtrace;
use std::fs;
use std::io;
use std::panic::{self, PanicHookInfo};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::system;
use crate::logging;

/// Names the report the next launch hasn't shown yet.
const PENDING_FILE: &str = "crash-pending";
/// Crash report file names start with this.
const REPORT_PREFIX: &str = "crash-";
/// Crash report file names end with this.
const REPORT_EXTENSION: &str = "txt";
/// How many reports are kept; older ones are deleted.
const REPORTS_KEPT: usize = 10;
/// The exit code after a crash, the same as an unhandled Rust panic's.
const CRASH_EXIT_CODE: i32 = 101;

/// Installs the panic hook. With no `logs` folder, panics are still logged and
/// printed, but no report is written.
pub fn install(logs: Option<PathBuf>) {
    let print_default = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let report = report(info, &Backtrace::force_capture());
        tracing::error!("crash: {}", message(info));
        if let Some(dir) = &logs {
            save_report(dir, &report);
        }
        print_default(info);
        std::process::exit(CRASH_EXIT_CODE);
    }));
}

/// Writes `report` into `dir` and logs where it went. Separate from the hook, with one
/// macro for both outcomes, because `tracing`'s `log` feature (on in Linux builds)
/// makes each macro add enough branches to push the hook past the complexity limit.
fn save_report(dir: &Path, report: &str) {
    let outcome = match save(dir, report, timestamp()) {
        Ok(path) => format!("report saved to {}", logging::path(&path)),
        Err(err) => format!("couldn't save the report: {err}"),
    };
    tracing::error!("crash: {outcome}");
}

/// The report the last run left behind, if it hasn't been shown yet. Taking it
/// marks it shown.
pub fn take_pending(logs: &Path) -> Option<PathBuf> {
    let marker = logs.join(PENDING_FILE);
    let name = fs::read_to_string(&marker).ok()?;
    fs::remove_file(&marker).ok();
    let report = logs.join(name.trim());
    report.is_file().then_some(report)
}

/// The report's text. The home directory is replaced by `~`.
fn report(info: &PanicHookInfo<'_>, backtrace: &Backtrace) -> String {
    let thread = std::thread::current();
    let location = info.location().map_or_else(|| "unknown".into(), ToString::to_string);
    let text = format!(
        "VLC crash report\n\
         Version: {}\nOS: {}\nGPU: {}\nlibVLC: {}\n\
         Thread: {}\nLocation: {location}\nMessage: {}\n\nBacktrace:\n{backtrace}\n",
        system::VERSION,
        system::os(),
        system::gpu(),
        system::loaded_libvlc_version(),
        thread.name().unwrap_or("unnamed"),
        message(info),
    );
    logging::redact_home(&text)
}

fn message(info: &PanicHookInfo<'_>) -> String {
    info.payload_as_str().unwrap_or("(no message)").to_owned()
}

/// Seconds since the Unix epoch, which sort the reports by time.
fn timestamp() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs())
}

/// Writes `report` to `dir`, marks it for the next launch, and deletes old reports.
fn save(dir: &Path, report: &str, time: u64) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let name = format!("{REPORT_PREFIX}{time}-{}.{REPORT_EXTENSION}", std::process::id());
    let path = dir.join(&name);
    fs::write(&path, report)?;
    fs::write(dir.join(PENDING_FILE), &name)?;
    prune(dir, REPORTS_KEPT);
    Ok(path)
}

/// Deletes all but the newest `keep` reports.
fn prune(dir: &Path, keep: usize) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut reports: Vec<(u64, PathBuf)> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter_map(|path| Some((report_time(&path)?, path)))
        .collect();
    reports.sort();
    let excess = reports.len().saturating_sub(keep);
    for (_, path) in reports.into_iter().take(excess) {
        fs::remove_file(path).ok();
    }
}

/// The time in a report's file name (`crash-<time>-<pid>.txt`).
fn report_time(path: &Path) -> Option<u64> {
    if path.extension()? != REPORT_EXTENSION {
        return None;
    }
    let stem = path.file_stem()?.to_str()?.strip_prefix(REPORT_PREFIX)?;
    stem.split('-').next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rvlc-crash-{name}-{}", std::process::id()));
        fs::remove_dir_all(&dir).ok();
        dir
    }

    #[test]
    fn the_next_launch_gets_the_report_once() {
        let dir = temp_dir("pending");
        let saved = save(&dir, "boom", 1_700_000_000).unwrap();
        assert_eq!(fs::read_to_string(&saved).unwrap(), "boom");
        assert_eq!(take_pending(&dir), Some(saved));
        assert_eq!(take_pending(&dir), None);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn only_the_newest_reports_are_kept() {
        let dir = temp_dir("prune");
        for time in 1..=4 {
            save(&dir, "boom", time).unwrap();
        }
        fs::write(dir.join("vlc.log"), "not a report").unwrap();
        prune(&dir, 2);
        let mut times: Vec<u64> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|entry| report_time(&entry.unwrap().path()))
            .collect();
        times.sort_unstable();
        assert_eq!(times, [3, 4]);
        assert!(dir.join("vlc.log").exists());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn no_marker_means_nothing_pending() {
        assert_eq!(take_pending(&temp_dir("none")), None);
    }
}
