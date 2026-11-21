//! Logging (S0.7.1): `tracing` events go to stderr and to a size-capped rolling file,
//! `logs/vlc.log` next to `local.toml` (see [`crate::settings::Locations::logs`]).
//!
//! - The level comes from `RUST_LOG` when it is set, otherwise from the
//!   `diagnostics.log_level` setting, which [`set_level`] applies while running.
//! - Records from the `log` crate (GPUI uses it) are forwarded to `tracing`.
//! - Paths are personal data: the file has the home directory replaced by `~`, and
//!   call sites log paths through [`path`], which hides file names when the
//!   "redact file names" privacy option is on.
//! - When the log folder can't be written, events go to stderr only.

mod level;
mod redact;
mod rolling;

use std::path::Path;
use std::sync::{Mutex, OnceLock};

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry, fmt, reload};

pub use level::LogLevel;
pub use redact::{path, redact_home, set_redact_file_names};

use redact::HomeRedactor;
use rolling::RollingFile;

/// The environment variable that overrides the level setting, in `EnvFilter` syntax.
const LEVEL_ENV: &str = "RUST_LOG";
/// Our crate's name as it appears in event targets.
const CRATE_TARGET: &str = env!("CARGO_CRATE_NAME");

/// Changes the filter while the app runs. Unset until [`init`] succeeds.
static FILTER: OnceLock<reload::Handle<EnvFilter, Registry>> = OnceLock::new();

/// Installs the global subscriber, writing to stderr and, when `log_dir` is given
/// and writable, to the rolling log file there. Does nothing if a subscriber is
/// already installed.
pub fn init(level: LogLevel, log_dir: Option<&Path>) {
    let (filter, handle) = reload::Layer::new(filter_for(level));
    let (file, file_error) = match log_dir.map(RollingFile::open) {
        Some(Ok(file)) => (Some(file), None),
        Some(Err(err)) => (None, Some(err)),
        None => (None, None),
    };
    let file_layer = file
        .map(|file| fmt::layer().with_writer(Mutex::new(HomeRedactor::new(file))).with_ansi(false));
    let installed = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(file_layer)
        .try_init();
    if installed.is_err() {
        return;
    }
    FILTER.set(handle).ok();
    if let (Some(dir), Some(err)) = (log_dir, file_error) {
        tracing::warn!("logging: can't write to {}; logging to stderr only: {err}", path(dir));
    }
}

/// Applies `level` from the settings, unless `RUST_LOG` overrides it.
pub fn set_level(level: LogLevel) {
    if level_from_env().is_some() {
        return;
    }
    if let Some(handle) = FILTER.get()
        && let Err(err) = handle.reload(level_filter(level))
    {
        tracing::warn!("logging: couldn't change the level: {err}");
    }
}

/// `RUST_LOG` when it is set and valid, else the filter for `level`.
fn filter_for(level: LogLevel) -> EnvFilter {
    level_from_env().unwrap_or_else(|| level_filter(level))
}

fn level_from_env() -> Option<EnvFilter> {
    let value = std::env::var(LEVEL_ENV).ok().filter(|value| !value.trim().is_empty())?;
    EnvFilter::try_new(value).ok()
}

/// Our events at `level`; other crates' (GPUI's) only from warnings up, since they
/// are chatty and rarely what a bug report needs.
fn level_filter(level: LogLevel) -> EnvFilter {
    EnvFilter::new(level::directive(level, CRATE_TARGET))
}
