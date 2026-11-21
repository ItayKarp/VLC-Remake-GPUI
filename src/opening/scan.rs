//! Opening a folder: the media files inside it and its subfolders, in the order a person
//! would list them ("Episode 2" before "Episode 10"), files before subfolders.
//!
//! The scan runs off the UI thread and stops when cancelled. It goes [`MAX_DEPTH`] folders
//! deep, stops at [`MAX_FILES`], skips hidden and system files, and follows a symlinked
//! folder only once (a link to a parent folder would loop forever otherwise). Playlists and
//! subtitles inside the folder are left out: the folder's media is the point, and libVLC finds
//! a video's subtitles next to it by itself.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::fs::{self, DirEntry};
use std::io;
use std::path::{Path, PathBuf};

use super::Cancel;
use super::kind::{self, Kind};

/// How many folders deep a scan goes below the one opened.
pub const MAX_DEPTH: usize = 8;
/// The most files one scan returns.
pub const MAX_FILES: usize = 10_000;

/// What a folder scan found.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Scan {
    /// The media files, in order.
    pub files: Vec<PathBuf>,
    /// Whether the scan stopped at [`MAX_FILES`].
    pub truncated: bool,
}

/// The media files in `folder`. Fails only when `folder` itself can't be read; unreadable
/// subfolders are skipped.
pub fn scan(folder: &Path, cancel: &Cancel) -> io::Result<Scan> {
    let mut scanner = Scanner { cancel, visited: HashSet::new(), scan: Scan::default() };
    let entries = read_sorted(folder)?;
    scanner.visit_entries(folder, entries, 0);
    Ok(scanner.scan)
}

struct Scanner<'a> {
    cancel: &'a Cancel,
    /// Folders already scanned, by canonical path.
    visited: HashSet<PathBuf>,
    scan: Scan,
}

impl Scanner<'_> {
    fn visit_entries(&mut self, folder: &Path, entries: Vec<DirEntry>, depth: usize) {
        if let Ok(canonical) = fs::canonicalize(folder)
            && !self.visited.insert(canonical)
        {
            return;
        }
        let mut subfolders = Vec::new();
        for entry in entries {
            if self.should_stop() {
                return;
            }
            // `metadata` follows symlinks, so a link to a file counts as that file.
            let Ok(metadata) = fs::metadata(entry.path()) else { continue };
            if metadata.is_dir() {
                subfolders.push(entry.path());
            } else if metadata.is_file() {
                self.consider(entry.path());
            }
        }
        if depth >= MAX_DEPTH {
            return;
        }
        for subfolder in subfolders {
            match read_sorted(&subfolder) {
                Ok(entries) => self.visit_entries(&subfolder, entries, depth + 1),
                Err(error) => {
                    tracing::debug!("scan: skipped {}: {error}", crate::logging::path(&subfolder));
                }
            }
        }
    }

    fn consider(&mut self, path: PathBuf) {
        if matches!(kind::classify(&path), Ok(Kind::Media)) {
            self.scan.files.push(path);
            self.scan.truncated = self.scan.files.len() >= MAX_FILES;
        }
    }

    fn should_stop(&self) -> bool {
        self.scan.truncated || self.cancel.is_cancelled()
    }
}

/// The visible entries of `folder`, in natural order.
fn read_sorted(folder: &Path) -> io::Result<Vec<DirEntry>> {
    let mut entries: Vec<DirEntry> =
        fs::read_dir(folder)?.filter_map(Result::ok).filter(|e| !is_hidden(e)).collect();
    entries.sort_by(|a, b| {
        natural_cmp(&a.file_name().to_string_lossy(), &b.file_name().to_string_lossy())
    });
    Ok(entries)
}

/// Dot files everywhere, and files Windows marks hidden or system.
fn is_hidden(entry: &DirEntry) -> bool {
    if entry.file_name().to_string_lossy().starts_with('.') {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const HIDDEN: u32 = 0x2;
        const SYSTEM: u32 = 0x4;
        if let Ok(metadata) = entry.metadata() {
            return metadata.file_attributes() & (HIDDEN | SYSTEM) != 0;
        }
    }
    false
}

/// Compares names the way people read them: runs of digits by their value, everything else
/// case-insensitively, so `Episode 2` comes before `Episode 10`.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a, b);
    while let (Some(x), Some(y)) = (a.chars().next(), b.chars().next()) {
        let order = if x.is_ascii_digit() && y.is_ascii_digit() {
            let (a_digits, a_rest) = split_digits(a);
            let (b_digits, b_rest) = split_digits(b);
            (a, b) = (a_rest, b_rest);
            compare_numbers(a_digits, b_digits)
        } else {
            (a, b) = (&a[x.len_utf8()..], &b[y.len_utf8()..]);
            x.to_lowercase().cmp(y.to_lowercase())
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    a.len().cmp(&b.len())
}

fn split_digits(text: &str) -> (&str, &str) {
    text.split_at(text.find(|c: char| !c.is_ascii_digit()).unwrap_or(text.len()))
}

/// Compares two digit runs by value, without overflowing on long ones.
fn compare_numbers(a: &str, b: &str) -> Ordering {
    let (a, b) = (a.trim_start_matches('0'), b.trim_start_matches('0'));
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opening::scratch::scratch_dir;

    fn names(scan: &Scan, root: &Path) -> Vec<String> {
        let relative =
            |p: &PathBuf| p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
        scan.files.iter().map(relative).collect()
    }

    #[test]
    fn natural_order() {
        let mut names = ["Episode 10.mkv", "episode 2.mkv", "Episode 1.mkv", "Extras", "E 007"];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(names, ["E 007", "Episode 1.mkv", "episode 2.mkv", "Episode 10.mkv", "Extras"]);
        assert_eq!(natural_cmp("a99999999999999999999999", "a1"), Ordering::Greater);
    }

    #[test]
    fn finds_media_in_order_and_skips_the_rest() {
        let root = scratch_dir();
        fs::create_dir_all(root.join("Season 1/.thumbs")).unwrap();
        for file in [
            "Season 1/E10.MKV",
            "Season 1/E2.mkv",
            "Season 1/.thumbs/x.mp4",
            "notes.txt",
            "a.srt",
            "b.m3u",
            ".hidden.mp4",
            "z.mp3",
        ] {
            fs::write(root.join(file), b"x").unwrap();
        }
        fs::write(root.join("no_extension"), b"\x1A\x45\xDF\xA3rest").unwrap();
        let scan = scan(&root, &Cancel::default()).unwrap();
        assert_eq!(
            names(&scan, &root),
            ["no_extension", "z.mp3", "Season 1/E2.mkv", "Season 1/E10.MKV"]
        );
        assert!(!scan.truncated);
    }

    #[test]
    fn stops_when_cancelled_and_fails_on_a_missing_folder() {
        let root = scratch_dir();
        fs::write(root.join("a.mp4"), b"x").unwrap();
        let cancel = Cancel::default();
        cancel.cancel();
        assert_eq!(scan(&root, &cancel).unwrap().files, Vec::<PathBuf>::new());
        assert!(scan(&root.join("missing"), &Cancel::default()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_loops_are_scanned_once() {
        let root = scratch_dir();
        fs::create_dir(root.join("sub")).unwrap();
        fs::write(root.join("sub/a.mp4"), b"x").unwrap();
        std::os::unix::fs::symlink(&root, root.join("sub/back")).unwrap();
        let scan = scan(&root, &Cancel::default()).unwrap();
        assert_eq!(names(&scan, &root), ["sub/a.mp4"]);
    }
}
