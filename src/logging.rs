//! A minimal stderr logger until real logging lands (S0.7).
//!
//! `RVLC_LOG=debug` (or `error`, `warn`, `info`, `trace`) sets the level; the default
//! is `warn`.

use log::{LevelFilter, Log, Metadata, Record};

/// Writes every enabled record to stderr.
struct StderrLogger;

impl Log for StderrLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            eprintln!("[{} {}] {}", record.level(), record.target(), record.args());
        }
    }

    fn flush(&self) {}
}

/// Installs the logger with the level from `RVLC_LOG`. Does nothing if a logger is
/// already installed.
pub fn init() {
    let level = std::env::var("RVLC_LOG")
        .ok()
        .and_then(|level| level.parse().ok())
        .unwrap_or(LevelFilter::Warn);
    if log::set_logger(&StderrLogger).is_ok() {
        log::set_max_level(level);
    }
}
