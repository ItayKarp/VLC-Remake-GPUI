//! Checks a local path before it reaches the backend, so a folder, a pipe, a device, an empty
//! file or a symlink loop is rejected with a clear error instead of hanging or confusing libVLC.

use std::fs;
use std::io;
use std::path::Path;

use super::state::ErrorKind;

/// Whether `path` is a non-empty regular file the backend can be given.
pub fn check(path: &Path) -> Result<(), ErrorKind> {
    // `metadata` follows symlinks, so a loop fails here rather than in the backend.
    let metadata = fs::metadata(path).map_err(|error| classify(&error))?;
    if !metadata.is_file() {
        return Err(ErrorKind::NotAFile);
    }
    if metadata.len() == 0 {
        return Err(ErrorKind::EmptyFile);
    }
    Ok(())
}

fn classify(error: &io::Error) -> ErrorKind {
    match error.kind() {
        io::ErrorKind::NotFound => ErrorKind::NotFound,
        _ => ErrorKind::Unreadable,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    /// A fresh, empty folder for one test.
    fn scratch_dir() -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "rvlc-preflight-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn accepts_a_regular_file() {
        let file = scratch_dir().join("clip.mkv");
        fs::write(&file, b"data").unwrap();
        assert_eq!(check(&file), Ok(()));
    }

    #[test]
    fn rejects_missing_empty_and_folder_paths() {
        let dir = scratch_dir();
        let empty = dir.join("empty.mp4");
        fs::write(&empty, b"").unwrap();
        assert_eq!(check(&dir.join("missing.mp4")), Err(ErrorKind::NotFound));
        assert_eq!(check(&empty), Err(ErrorKind::EmptyFile));
        assert_eq!(check(&dir), Err(ErrorKind::NotAFile));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_devices_pipes_and_symlink_loops() {
        let dir = scratch_dir();
        let fifo = dir.join("pipe");
        let made = std::process::Command::new("mkfifo").arg(&fifo).status().unwrap();
        assert!(made.success());
        let looped = dir.join("loop");
        std::os::unix::fs::symlink(&looped, &looped).unwrap();
        assert_eq!(check(Path::new("/dev/null")), Err(ErrorKind::NotAFile));
        assert_eq!(check(&fifo), Err(ErrorKind::NotAFile));
        assert_eq!(check(&looped), Err(ErrorKind::Unreadable));
    }
}
