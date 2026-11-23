//! Recording (S8.6): where recordings go, whether there is room, and finding the file.
//!
//! Recordings go to Settings → Recording's "Save recordings to", else the OS Videos folder
//! (`~/Movies` on macOS). A missing folder is made; one that can't be written gives way to the
//! Videos folder, then home, then the temporary folder. libVLC names the file itself, so once a
//! recording ends the newest `vlc-record-…` file in the folder that changed since it started is
//! the one.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::settings::expand_home;

/// How libVLC starts a recording's file name.
const FILE_PREFIX: &str = "vlc-record-";
/// The least free space a recording starts with.
pub const START_SPACE: u64 = 256 * 1024 * 1024;
/// Below this much free space a running recording is stopped, so its file can still be
/// finished.
pub const STOP_SPACE: u64 = 64 * 1024 * 1024;
/// How much earlier than the recording's start its file may say it changed: file systems keep
/// times coarsely (FAT: two seconds).
const CLOCK_SLACK: Duration = Duration::from_secs(2);
/// The file made and removed to check a folder can be written.
const PROBE_NAME: &str = ".rvlc-write-check";

/// Where a recording goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Destination {
    /// The folder it is recorded into.
    pub folder: PathBuf,
    /// The folder the setting named, when it couldn't be written and this is another.
    pub instead_of: Option<PathBuf>,
}

/// Why a recording can't start.
#[derive(Debug)]
pub enum Unready {
    /// No folder could be written; the chosen one's error.
    Unwritable(io::Error),
    /// The folder has less than [`START_SPACE`] free.
    NoSpace(PathBuf),
}

/// The folder recordings go to while the setting is empty: the OS Videos folder.
pub fn default_folder() -> PathBuf {
    let videos = if cfg!(target_os = "macos") { "Movies" } else { "Videos" };
    dirs::video_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join(videos)))
        .unwrap_or_else(std::env::temp_dir)
}

/// Where to try recording, in order: the folder `setting` ("Save recordings to", as typed)
/// names, then the Videos folder, home and the temporary folder, each once.
pub fn candidates(setting: &str) -> Vec<PathBuf> {
    let setting = setting.trim();
    let chosen = if setting.is_empty() { default_folder() } else { expand_home(setting) };
    let mut folders = vec![chosen];
    let fallbacks = [Some(default_folder()), dirs::home_dir(), Some(std::env::temp_dir())];
    for folder in fallbacks.into_iter().flatten() {
        if !folders.contains(&folder) {
            folders.push(folder);
        }
    }
    folders
}

/// The first of `candidates` that can be written, made if missing, with room to record. Blocks
/// on the file system: run it off the UI thread.
pub fn prepare(candidates: &[PathBuf]) -> Result<Destination, Unready> {
    let mut first_error = None;
    for folder in candidates {
        match writable(folder) {
            Ok(()) if has_room(folder, START_SPACE) => {
                let instead_of = (folder != &candidates[0]).then(|| candidates[0].clone());
                return Ok(Destination { folder: folder.clone(), instead_of });
            }
            Ok(()) => return Err(Unready::NoSpace(folder.clone())),
            Err(error) => {
                tracing::warn!(
                    "recording: can't write to {}: {error}",
                    crate::logging::path(folder)
                );
                first_error.get_or_insert(error);
            }
        }
    }
    Err(Unready::Unwritable(first_error.unwrap_or_else(|| io::Error::other("no folder"))))
}

/// Makes `folder` if it is missing and checks a file can be made in it.
fn writable(folder: &Path) -> io::Result<()> {
    fs::create_dir_all(folder)?;
    let probe = folder.join(PROBE_NAME);
    fs::OpenOptions::new().write(true).create(true).truncate(true).open(&probe)?;
    fs::remove_file(probe)
}

/// Whether `folder`'s disk has at least `bytes` free; true when that can't be told.
pub fn has_room(folder: &Path, bytes: u64) -> bool {
    fs4::available_space(folder).map_or(true, |free| free >= bytes)
}

/// The recording made in `folder` since `since`: the newest `vlc-record-…` file changed since
/// then. `None` when there is none, as when a recording stopped before its first keyframe.
pub fn find(folder: &Path, since: SystemTime) -> Option<PathBuf> {
    let since = since.checked_sub(CLOCK_SLACK).unwrap_or(since);
    fs::read_dir(folder)
        .ok()?
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(FILE_PREFIX))
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .filter(|(modified, _)| *modified >= since)
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_folder(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("rvlc-recording-unit-{name}"));
        _ = fs::remove_dir_all(&folder);
        folder
    }

    #[test]
    fn the_chosen_folder_comes_first_and_none_twice() {
        let folders = candidates("/somewhere/else");
        assert_eq!(folders[0], PathBuf::from("/somewhere/else"));
        assert_eq!(folders[1], default_folder());
        assert_eq!(candidates(" ")[0], default_folder());
        let mut unique = folders.clone();
        unique.dedup();
        assert_eq!(unique.len(), folders.len());
    }

    #[test]
    fn a_missing_folder_is_made() {
        let folder = temp_folder("missing").join("inside");
        let destination = prepare(std::slice::from_ref(&folder)).expect("a destination");
        assert_eq!(destination, Destination { folder: folder.clone(), instead_of: None });
        assert!(folder.is_dir());
        assert!(!folder.join(PROBE_NAME).exists());
    }

    #[test]
    fn a_folder_that_cant_be_made_gives_way_to_the_next() {
        let parent = temp_folder("blocked");
        fs::create_dir_all(&parent).unwrap();
        let file = parent.join("a-file");
        fs::write(&file, b"").unwrap();
        let blocked = file.join("folder");
        let destination = prepare(&[blocked.clone(), parent.clone()]).expect("a destination");
        assert_eq!(destination, Destination { folder: parent, instead_of: Some(blocked) });
    }

    #[test]
    fn the_newest_recording_since_the_start_is_found() {
        let folder = temp_folder("find");
        fs::create_dir_all(&folder).unwrap();
        let start = SystemTime::now();
        assert_eq!(find(&folder, start), None);
        fs::write(folder.join("other.mp4"), b"x").unwrap();
        assert_eq!(find(&folder, start), None);
        let recording = folder.join("vlc-record-2026-11-23-20h01m46s-clip.mp4-.mp4");
        fs::write(&recording, b"x").unwrap();
        assert_eq!(find(&folder, start), Some(recording));
        let later = start + Duration::from_secs(60);
        assert_eq!(find(&folder, later), None);
    }
}
