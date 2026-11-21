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

/// An `EnvFilter` directive: `crate_target` at `level`, other crates at `level` but
/// no more verbose than warnings.
pub(super) fn directive(level: LogLevel, crate_target: &str) -> String {
    let others = level.min(LogLevel::Warn);
    format!("{},{crate_target}={}", others.as_str(), level.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn other_crates_stay_at_warnings_when_ours_are_verbose() {
        assert_eq!(directive(LogLevel::Debug, "vlc"), "warn,vlc=debug");
        assert_eq!(directive(LogLevel::Error, "vlc"), "error,vlc=error");
    }

    #[test]
    fn the_setting_reads_lowercase_names() {
        let level: LogLevel = toml::Value::String("debug".into()).try_into().unwrap();
        assert_eq!(level, LogLevel::Debug);
    }
}
