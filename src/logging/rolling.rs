//! A log file that rolls over by size: `vlc.log` is the newest, then `vlc.1.log`
//! and so on, and the oldest is deleted once there are [`FILE_COUNT`] of them.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// The newest log file's name.
const FILE_NAME: &str = "vlc.log";
/// How large one file grows before it rolls over.
const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
/// How many files are kept, the current one included.
const FILE_COUNT: usize = 5;

/// A size-capped, rolling log file.
pub(super) struct RollingFile {
    dir: PathBuf,
    file: File,
    written: u64,
    max_bytes: u64,
    count: usize,
}

impl RollingFile {
    /// Opens (or creates) `vlc.log` in `dir`, appending to it.
    pub(super) fn open(dir: &Path) -> io::Result<Self> {
        Self::with_limits(dir, MAX_FILE_BYTES, FILE_COUNT)
    }

    fn with_limits(dir: &Path, max_bytes: u64, count: usize) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        let file = OpenOptions::new().create(true).append(true).open(dir.join(FILE_NAME))?;
        let written = file.metadata()?.len();
        Ok(Self { dir: dir.to_owned(), file, written, max_bytes, count })
    }

    fn name(&self, index: usize) -> PathBuf {
        if index == 0 {
            self.dir.join(FILE_NAME)
        } else {
            self.dir.join(format!("vlc.{index}.log"))
        }
    }

    /// Shifts every file up by one, dropping the oldest, and starts a new one.
    fn roll_over(&mut self) -> io::Result<()> {
        let oldest = self.name(self.count - 1);
        if oldest.exists() {
            fs::remove_file(oldest)?;
        }
        for index in (0..self.count - 1).rev() {
            let from = self.name(index);
            if from.exists() {
                fs::rename(from, self.name(index + 1))?;
            }
        }
        self.file = File::create(self.name(0))?;
        self.written = 0;
        Ok(())
    }
}

impl Write for RollingFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let len = buf.len() as u64;
        // A failed roll-over (another process holding a file) keeps writing to the
        // current file rather than losing the message.
        if self.written > 0 && self.written + len > self.max_bytes && self.count > 1 {
            self.roll_over().ok();
        }
        self.file.write_all(buf)?;
        self.written += len;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rvlc-rolling-{name}-{}", std::process::id()));
        fs::remove_dir_all(&dir).ok();
        dir
    }

    #[test]
    fn rolls_over_at_the_size_cap_and_keeps_a_fixed_number_of_files() {
        let dir = temp_dir("cap");
        let mut log = RollingFile::with_limits(&dir, 10, 3).unwrap();
        for line in ["aaaaaaaa\n", "bbbbbbbb\n", "cccccccc\n", "dddddddd\n"] {
            log.write_all(line.as_bytes()).unwrap();
        }
        let read = |name: &str| fs::read_to_string(dir.join(name)).unwrap();
        assert_eq!(read("vlc.log"), "dddddddd\n");
        assert_eq!(read("vlc.1.log"), "cccccccc\n");
        assert_eq!(read("vlc.2.log"), "bbbbbbbb\n");
        assert!(!dir.join("vlc.3.log").exists());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn appends_to_the_existing_file_and_counts_its_size() {
        let dir = temp_dir("append");
        RollingFile::with_limits(&dir, 10, 2).unwrap().write_all(b"12345678\n").unwrap();
        RollingFile::with_limits(&dir, 10, 2).unwrap().write_all(b"abc\n").unwrap();
        assert_eq!(fs::read_to_string(dir.join("vlc.log")).unwrap(), "abc\n");
        assert_eq!(fs::read_to_string(dir.join("vlc.1.log")).unwrap(), "12345678\n");
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn an_unwritable_folder_is_an_error() {
        let dir = temp_dir("blocked");
        fs::create_dir_all(dir.parent().unwrap()).unwrap();
        fs::write(&dir, b"a file, not a folder").unwrap();
        assert!(RollingFile::open(&dir).is_err());
        fs::remove_file(dir).ok();
    }
}
