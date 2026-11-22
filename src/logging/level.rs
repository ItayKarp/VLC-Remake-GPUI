//! The log level setting and the filter directive it becomes.

use serde::{Deserialize, Serialize};

/// How much the app logs, from least to most.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// Only failures.
    Error,
    /// Failures and things that went wrong but were recovered from.
    Warn,
    /// Also notable events (settings migrated, layout changed).
    #[default]
    Info,
    /// Also details useful when chasing a bug.
    Debug,
    /// Everything.
    Trace,
}

impl LogLevel {
    /// The level's name in filter directives.
    pub fn as_str(self) -> &'static str {
        match self {
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
            LogLevel::Trace => "trace",
        }
    }
}

/// An `EnvFilter` directive: `crate_target` at `level`, other crates (GPUI) at `level` but no
/// more verbose than warnings with `minor_warnings`, and errors only without.
pub(super) fn directive(level: LogLevel, minor_warnings: bool, crate_target: &str) -> String {
    let loudest = if minor_warnings { LogLevel::Warn } else { LogLevel::Error };
    let others = level.min(loudest);
    format!("{},{crate_target}={}", others.as_str(), level.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn other_crates_stay_at_warnings_when_ours_are_verbose() {
        assert_eq!(directive(LogLevel::Debug, true, "vlc"), "warn,vlc=debug");
        assert_eq!(directive(LogLevel::Error, true, "vlc"), "error,vlc=error");
    }

    #[test]
    fn without_minor_warnings_other_crates_log_only_errors() {
        assert_eq!(directive(LogLevel::Debug, false, "vlc"), "error,vlc=debug");
    }

    #[test]
    fn the_setting_reads_lowercase_names() {
        let level: LogLevel = toml::Value::String("debug".into()).try_into().unwrap();
        assert_eq!(level, LogLevel::Debug);
    }
}
