//! Keeping personal data out of logs: the home directory becomes `~` in everything
//! written to the log file, and [`path`] hides file names when asked to.

use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// The "redact file names" privacy option, mirrored here so logging never needs
/// the settings global.
static REDACT_FILE_NAMES: AtomicBool = AtomicBool::new(false);

/// What a redacted file name is replaced with; the extension is kept because it
/// helps with format bugs and says nothing about the user.
const REDACTED: &str = "<redacted>";

/// Turns hiding file names in logged paths on or off.
pub fn set_redact_file_names(on: bool) {
    REDACT_FILE_NAMES.store(on, Ordering::Relaxed);
}

/// A path for a log message: `<redacted>.mkv` when file names are redacted,
/// otherwise the path itself (the log file still replaces the home directory).
pub fn path(path: &Path) -> LoggedPath<'_> {
    LoggedPath { path, redact: REDACT_FILE_NAMES.load(Ordering::Relaxed) }
}

/// Displays a path as [`path`] decided.
pub struct LoggedPath<'a> {
    path: &'a Path,
    redact: bool,
}

impl fmt::Display for LoggedPath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.redact {
            return write!(f, "{}", self.path.display());
        }
        f.write_str(REDACTED)?;
        match self.path.extension() {
            Some(extension) => write!(f, ".{}", extension.display()),
            None => Ok(()),
        }
    }
}

/// Writes to `inner` with the home directory replaced by `~`.
pub(super) struct HomeRedactor<W> {
    inner: W,
    /// The home directory as it appears in text, with native and `/` separators.
    patterns: Vec<String>,
}

impl<W: Write> HomeRedactor<W> {
    /// Redacts the current user's home directory.
    pub(super) fn new(inner: W) -> Self {
        Self::with_home(inner, dirs::home_dir())
    }

    fn with_home(inner: W, home: Option<PathBuf>) -> Self {
        // A home of `/` or `C:\` would turn every path into `~`, hiding the parts
        // that are useful; only redact a real home folder.
        let patterns = home
            .filter(|home| home.components().count() > 1 && home.parent().is_some())
            .map(|home| {
                let native = home.display().to_string();
                let slashed = native.replace('\\', "/");
                if slashed == native { vec![native] } else { vec![native, slashed] }
            })
            .unwrap_or_default();
        Self { inner, patterns }
    }

    fn redact(&self, text: &str) -> String {
        self.patterns.iter().fold(text.to_owned(), |text, home| text.replace(home.as_str(), "~"))
    }
}

impl<W: Write> Write for HomeRedactor<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        self.inner.write_all(self.redact(&text).as_bytes())?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Replaces the current user's home directory in `text` with `~`.
pub fn redact_home(text: &str) -> String {
    HomeRedactor::new(io::sink()).redact(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> PathBuf {
        if cfg!(windows) { PathBuf::from(r"C:\Users\pat") } else { PathBuf::from("/home/pat") }
    }

    #[test]
    fn the_home_directory_becomes_a_tilde() {
        let mut out = Vec::new();
        let mut writer = HomeRedactor::with_home(&mut out, Some(home()));
        let line = format!("opened {}\n", home().join("Videos").join("a.mkv").display());
        writer.write_all(line.as_bytes()).unwrap();
        let written = String::from_utf8(out).unwrap();
        assert!(!written.contains("pat"), "{written}");
        assert!(written.starts_with("opened ~"), "{written}");
    }

    // A Windows home: on Unix `C:\Users\pat` is one file name, not a folder.
    #[cfg(windows)]
    #[test]
    fn forward_slash_spellings_are_redacted_too() {
        let writer = HomeRedactor::with_home(io::sink(), Some(PathBuf::from(r"C:\Users\pat")));
        assert_eq!(writer.redact("file:///C:/Users/pat/a.mkv"), "file:///~/a.mkv");
    }

    #[test]
    fn a_root_home_is_left_alone() {
        let root = if cfg!(windows) { r"C:\" } else { "/" };
        let writer = HomeRedactor::with_home(io::sink(), Some(PathBuf::from(root)));
        assert_eq!(writer.redact("/etc/hosts"), "/etc/hosts");
    }

    #[test]
    fn redacted_paths_keep_only_the_extension() {
        let movie = home().join("secret name.mkv");
        let shown = LoggedPath { path: &movie, redact: true }.to_string();
        assert_eq!(shown, "<redacted>.mkv");
        let shown = LoggedPath { path: &movie, redact: false }.to_string();
        assert!(shown.ends_with("secret name.mkv"));
    }
}
