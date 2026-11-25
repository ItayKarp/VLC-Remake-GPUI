//! Logging (S0.7.1, S5.3.4): `tracing` events go to stderr and to a size-capped rolling file,
//! `logs/vlc.log` next to `local.toml` (see [`crate::settings::Locations::logs`]) or the file
//! the user picked.
//!
//! - The level comes from `RUST_LOG` when it is set, otherwise from the
//!   `diagnostics.log_level` setting; [`configure`] applies the settings while running.
//! - Records from the `log` crate (GPUI uses it) are forwarded to `tracing`. Their warnings are
//!   logged only with "Show minor warnings"; their errors always are.
//! - "Save a log" off writes no file; "Log format" writes it as plain text or JSON lines.
//! - Paths are personal data: the file has the home directory replaced by `~`, and
//!   call sites log paths through [`path`], which hides file names when the
//!   "redact file names" privacy option is on.
//! - When the log file can't be written, the default one is used, and failing that,
//!   events go to stderr only.

mod file;
mod format;
mod level;
mod redact;
mod rolling;

use std::path::Path;
use std::sync::OnceLock;

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry, fmt, reload};

pub use file::{LogFile, check as check_file};
pub use format::LogFormat;
pub use level::LogLevel;
pub use redact::{path, redact_home, redacts_file_names, set_redact_file_names};

use file::CurrentFile;
use format::FileFormat;

/// The environment variable that overrides the level setting, in `EnvFilter` syntax.
const LEVEL_ENV: &str = "RUST_LOG";
/// Our crate's name as it appears in event targets.
const CRATE_TARGET: &str = env!("CARGO_CRATE_NAME");

/// Changes the filter while the app runs. Unset until [`init`] succeeds.
static FILTER: OnceLock<reload::Handle<EnvFilter, Registry>> = OnceLock::new();

/// What the log settings ask for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogOptions {
    /// How much our code logs.
    pub level: LogLevel,
    /// Whether other components' warnings are logged, not only their errors.
    pub minor_warnings: bool,
    /// The file to write, or `None` for stderr only.
    pub file: Option<LogFile>,
    /// How the file is written.
    pub format: LogFormat,
}

/// The default log file in `dir`.
pub fn default_file(dir: &Path) -> std::path::PathBuf {
    dir.join(rolling::FILE_NAME)
}

/// Installs the global subscriber, writing to stderr and to the file `options` name. Does
/// nothing if a subscriber is already installed.
pub fn init(options: &LogOptions) {
    let (filter, handle) = reload::Layer::new(filter_for(options));
    format::set(options.format);
    let file_layer =
        fmt::layer().event_format(FileFormat::new()).with_writer(|| CurrentFile).with_ansi(false);
    let installed = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(file_layer)
        .try_init();
    if installed.is_err() {
        return;
    }
    FILTER.set(handle).ok();
    if let Some(warning) = file::open_now(options.file.clone()) {
        tracing::warn!("logging: {warning}");
    }
}

/// Applies the log settings while the app runs. The level is left alone while `RUST_LOG` sets
/// it; a new file opens in the background.
pub fn configure(options: &LogOptions) {
    format::set(options.format);
    file::set(options.file.clone());
    if level_from_env().is_some() {
        return;
    }
    if let Some(handle) = FILTER.get()
        && let Err(err) = handle.reload(level_filter(options))
    {
        tracing::warn!("logging: couldn't change the level: {err}");
    }
}

/// `RUST_LOG` when it is set and valid, else the filter for `options`.
fn filter_for(options: &LogOptions) -> EnvFilter {
    level_from_env().unwrap_or_else(|| level_filter(options))
}

fn level_from_env() -> Option<EnvFilter> {
    let value = std::env::var(LEVEL_ENV).ok().filter(|value| !value.trim().is_empty())?;
    EnvFilter::try_new(value).ok()
}

/// Our events at the chosen level; other crates' (GPUI's) no more verbose than warnings, or
/// errors without minor warnings, since they are chatty and rarely what a bug report needs.
fn level_filter(options: &LogOptions) -> EnvFilter {
    EnvFilter::new(level::directive(options.level, options.minor_warnings, CRATE_TARGET))
}
