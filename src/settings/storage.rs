//! Crash-safe reading and writing of one settings file (S0.1.3 and the edge cases).
//!
//! Save: take an advisory lock, write `<file>.tmp` and fsync it, copy the current file
//! to `<file>.bak`, then rename the temp file over the real one. A crash at any point
//! leaves either the old or the new file in place, never a half-written one.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use toml_edit::DocumentMut;

/// What happened while loading, for a one-time notice to the user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadIssue {
    /// The file was empty (a crash on a filesystem without atomic rename) and the
    /// backup was used instead.
    RestoredFromBackup,
    /// The file didn't parse. It was moved aside to `moved_to` (if that worked), and
    /// the backup was used if it parsed, otherwise defaults.
    Corrupt { error: String, moved_to: Option<PathBuf>, restored_from_backup: bool },
}

#[derive(Debug)]
pub struct Loaded {
    /// Empty if the file doesn't exist.
    pub doc: DocumentMut,
    pub issue: Option<LoadIssue>,
}

/// Loads `path`. An error means the file exists but can't be read (permission
/// denied, for example); the caller must then not overwrite it.
pub fn load(path: &Path) -> io::Result<Loaded> {
    let path = &resolve_symlink(path);
    let text = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(err) => return Err(err),
    };

    if text.iter().all(u8::is_ascii_whitespace) {
        // Missing or 0-byte: fall back to the backup if there is one.
        let existed = path.exists();
        return Ok(match load_backup(path) {
            Some(doc) => Loaded { doc, issue: existed.then_some(LoadIssue::RestoredFromBackup) },
            None => Loaded { doc: DocumentMut::new(), issue: None },
        });
    }

    match parse(&text) {
        Ok(doc) => Ok(Loaded { doc, issue: None }),
        Err(error) => {
            log::warn!("settings: {} is corrupt: {error}", path.display());
            let moved_to = move_aside(path);
            let backup = load_backup(path);
            Ok(Loaded {
                issue: Some(LoadIssue::Corrupt {
                    error,
                    moved_to,
                    restored_from_backup: backup.is_some(),
                }),
                doc: backup.unwrap_or_default(),
            })
        }
    }
}

fn parse(bytes: &[u8]) -> Result<DocumentMut, String> {
    let text = std::str::from_utf8(bytes).map_err(|err| err.to_string())?;
    text.parse::<DocumentMut>().map_err(|err| err.to_string())
}

fn load_backup(path: &Path) -> Option<DocumentMut> {
    let bytes = fs::read(sibling(path, ".bak")).ok()?;
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return None;
    }
    parse(&bytes).ok()
}

/// Renames a corrupt file to `<stem>.corrupt-<timestamp>.toml` so the user's data isn't
/// lost. The timestamp is only a label; nothing relies on it for ordering.
fn move_aside(path: &Path) -> Option<PathBuf> {
    let stem = path.file_stem()?.to_string_lossy().into_owned();
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let dir = path.parent()?;
    let target = (0..100)
        .map(|n| match n {
            0 => dir.join(format!("{stem}.corrupt-{secs}.toml")),
            n => dir.join(format!("{stem}.corrupt-{secs}-{n}.toml")),
        })
        .find(|candidate| !candidate.exists())?;
    match with_retry(|| fs::rename(path, &target)) {
        Ok(()) => Some(target),
        Err(err) => {
            log::warn!("settings: couldn't move corrupt file aside: {err}");
            None
        }
    }
}

/// Atomically replaces `path` with `contents`. Skips the write if nothing changed.
pub fn save(path: &Path, contents: &str) -> io::Result<()> {
    let path = &resolve_symlink(path);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }

    // Backstop against two instances writing at once (single instance is S0.6).
    let lock =
        OpenOptions::new().create(true).truncate(false).write(true).open(sibling(path, ".lock"))?;
    lock.lock()?;

    let current = fs::read(path).ok();
    if current.as_deref() == Some(contents.as_bytes()) {
        return Ok(());
    }

    let tmp = sibling(path, ".tmp");
    write_synced(&tmp, contents.as_bytes())?;

    if let Some(current) = current.filter(|bytes| !bytes.iter().all(u8::is_ascii_whitespace)) {
        // Best effort: a failed backup must not block saving the new settings.
        let bak_tmp = sibling(path, ".bak.tmp");
        let backup = write_synced(&bak_tmp, &current)
            .and_then(|()| with_retry(|| fs::rename(&bak_tmp, sibling(path, ".bak"))));
        if let Err(err) = backup {
            log::warn!("settings: couldn't update backup of {}: {err}", path.display());
        }
    }

    with_retry(|| fs::rename(&tmp, path))?;
    sync_dir(path);
    Ok(())
}

fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// `settings.toml` + `.bak` → `settings.toml.bak`.
fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(suffix);
    path.with_file_name(name)
}

/// If `path` is a symlink (a dotfiles repo, for example), returns its target so the
/// link itself is never replaced by a regular file.
fn resolve_symlink(path: &Path) -> PathBuf {
    let is_link = fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink());
    if is_link && let Ok(target) = fs::canonicalize(path) {
        return target;
    }
    path.to_owned()
}

/// Retries operations that antivirus or indexers can briefly block on Windows.
fn with_retry<T>(mut op: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    const ATTEMPTS: u32 = 6;
    let mut delay = Duration::from_millis(10);
    for _ in 1..ATTEMPTS {
        match op() {
            Err(err) if is_transient(&err) => {
                std::thread::sleep(delay);
                delay *= 2;
            }
            result => return result,
        }
    }
    op()
}

fn is_transient(err: &io::Error) -> bool {
    // ERROR_ACCESS_DENIED (5), ERROR_SHARING_VIOLATION (32), ERROR_LOCK_VIOLATION (33).
    cfg!(windows) && matches!(err.raw_os_error(), Some(5 | 32 | 33))
}

/// Makes the rename durable on Unix. Windows has no directory fsync.
fn sync_dir(path: &Path) {
    #[cfg(unix)]
    if let Some(dir) = path.parent()
        && let Ok(dir) = File::open(dir)
    {
        let _ = dir.sync_all();
    }
    #[cfg(not(unix))]
    let _ = path;
}
