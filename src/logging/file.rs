//! The log file the file layer writes to, which "Save a log" and "Log file" (S5.3.4) change while
//! the app runs.
//!
//! A new file is opened on a thread of its own: the path may be on a network share that is slow
//! to answer. When it can't be opened (a missing or read-only folder, a share that is gone, a
//! file another program holds), the default file is used instead, with a warning. A later write
//! failing (the share disconnects) loses that event only.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use super::path;
use super::redact::HomeRedactor;
use super::rolling::RollingFile;

/// Where the log goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogFile {
    /// The file asked for.
    pub path: PathBuf,
    /// The file used when `path` can't be opened; the same as `path` for the default.
    pub fallback: PathBuf,
}

/// The file being written, and the target it was opened for.
struct Current {
    target: Option<LogFile>,
    writer: Option<HomeRedactor<RollingFile>>,
}

static CURRENT: Mutex<Current> = Mutex::new(Current { target: None, writer: None });
/// Counts [`set`] calls, so a slow open doesn't replace a file asked for after it.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Writes each event to the current log file, or nowhere while there is none.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct CurrentFile;

impl Write for CurrentFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut current = CURRENT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        match current.writer.as_mut() {
            Some(writer) => writer.write(buf),
            None => Ok(buf.len()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut current = CURRENT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        current.writer.as_mut().map_or(Ok(()), Write::flush)
    }
}

/// Opens `target` now, on this thread, for startup. Returns why it fell back, if it did.
pub(super) fn open_now(target: Option<LogFile>) -> Option<String> {
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    install(generation, target)
}

/// Writes to `target` from now on (`None`: no file). Opening happens in the background; nothing
/// happens when `target` is already the current one.
pub fn set(target: Option<LogFile>) {
    let current = CURRENT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if current.target == target {
        return;
    }
    drop(current);
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let spawned = std::thread::Builder::new().name("log-file".into()).spawn(move || {
        if let Some(warning) = install(generation, target) {
            tracing::warn!("logging: {warning}");
        }
    });
    if let Err(err) = spawned {
        tracing::warn!("logging: couldn't switch the log file: {err}");
    }
}

/// Opens `target` and makes it current unless a newer [`set`] came meanwhile.
fn install(generation: u64, target: Option<LogFile>) -> Option<String> {
    let (writer, warning) = match &target {
        Some(target) => open(target),
        None => (None, None),
    };
    let mut current = CURRENT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if GENERATION.load(Ordering::SeqCst) == generation {
        *current = Current { target, writer: writer.map(HomeRedactor::new) };
    }
    warning
}

/// The file for `target`: its path, else its fallback, with a warning saying why. `None` when
/// neither can be opened.
fn open(target: &LogFile) -> (Option<RollingFile>, Option<String>) {
    let error = match RollingFile::open(&target.path) {
        Ok(file) => return (Some(file), None),
        Err(error) => error,
    };
    let reason = format!("can't write to {}: {error}", path(&target.path));
    if target.fallback == target.path {
        return (None, Some(format!("{reason}; logging to stderr only")));
    }
    match RollingFile::open(&target.fallback) {
        Ok(file) => {
            let fallback = path(&target.fallback);
            (Some(file), Some(format!("{reason}; using {fallback} instead")))
        }
        Err(_) => (None, Some(format!("{reason}; logging to stderr only"))),
    }
}

/// Whether `path` can take the log: an absolute path to a file, not a folder, whose folder
/// exists. Says why not otherwise. Opening it is left to [`set`], which falls back if needed.
pub fn check(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("Choose a full path, such as one from Browse".into());
    }
    if path.is_dir() {
        return Err("That's a folder; choose a file".into());
    }
    match path.parent() {
        Some(dir) if dir.is_dir() => Ok(()),
        _ => Err("That folder doesn't exist".into()),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rvlc-logfile-{name}-{}", std::process::id()));
        fs::remove_dir_all(&dir).ok();
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn an_unwritable_file_falls_back_to_the_default_with_a_warning() {
        let dir = temp_dir("fallback");
        let blocked = dir.join("not-a-folder");
        fs::write(&blocked, b"a file").unwrap();
        let target = LogFile {
            path: blocked.join("vlc.log"),
            fallback: dir.join("default").join("vlc.log"),
        };
        let (file, warning) = open(&target);
        assert!(file.is_some());
        assert!(warning.is_some_and(|warning| warning.contains("instead")));
        assert!(dir.join("default").join("vlc.log").exists());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_good_file_opens_without_a_warning() {
        let dir = temp_dir("good");
        let target = LogFile { path: dir.join("mine.log"), fallback: dir.join("vlc.log") };
        let (file, warning) = open(&target);
        assert!(file.is_some() && warning.is_none());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn checks_name_what_is_wrong_with_a_path() {
        let dir = temp_dir("check");
        assert!(check(&dir.join("vlc.log")).is_ok());
        assert!(check(Path::new("relative.log")).is_err());
        assert_eq!(check(&dir), Err("That's a folder; choose a file".into()));
        assert_eq!(
            check(&dir.join("missing").join("vlc.log")),
            Err("That folder doesn't exist".into())
        );
        fs::remove_dir_all(dir).ok();
    }
}
