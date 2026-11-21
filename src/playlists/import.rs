//! Importing a playlist file as a saved playlist (S3.6.2): `.m3u`, `.m3u8`, `.pls`, `.xspf`,
//! `.cue` and `.asx`, read the way opening one reads it ([`crate::opening::playlist`]).
//!
//! One bad entry never fails the import: URLs are kept, nested local playlists are expanded,
//! repeated entries are kept (a playlist may repeat a song on purpose), and entries whose file
//! is missing are kept and counted, so the user is told and the item can be found again later.

use std::io;
use std::path::Path;

use crate::app::media::Media;
use crate::opening::kind::{self, Kind};
use crate::opening::playlist::{self, PlaylistError};
use crate::opening::{Cancel, Problem};

/// A playlist file, read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Imported {
    /// The title to save it under: the file's name without its extension.
    pub title: String,
    /// Its items, in order.
    pub items: Vec<Media>,
    /// How many of them are local files that don't exist.
    pub missing: usize,
}

impl Imported {
    /// What the user is told once it is saved as `title`.
    pub fn message(&self, title: &str) -> String {
        let count = match self.items.len() {
            1 => "1 item".to_owned(),
            n => format!("{n} items"),
        };
        let missing = match self.missing {
            0 => String::new(),
            1 => " 1 file is missing.".to_owned(),
            n => format!(" {n} files are missing."),
        };
        format!("Imported “{title}” ({count}).{missing}")
    }
}

/// Why a file couldn't be imported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportError {
    /// It isn't a playlist file.
    NotAPlaylist,
    /// It is one, but couldn't be read.
    Unreadable(Problem),
}

impl ImportError {
    /// The message for the file at `path` failing this way.
    pub fn message(self, path: &Path) -> String {
        let media = Media::File(path.to_path_buf());
        match self {
            ImportError::NotAPlaylist => format!("“{}” isn't a playlist.", media.display_name()),
            ImportError::Unreadable(problem) => problem.message(&media),
        }
    }
}

/// Reads the playlist file at `path`. Reads files, so call it off the UI thread.
pub fn read(path: &Path, cancel: &Cancel) -> Result<Imported, ImportError> {
    let unreadable = |kind: io::ErrorKind| ImportError::Unreadable(Problem::Unreadable(kind));
    let Kind::Playlist(format) = kind::classify(path).map_err(|e| unreadable(e.kind()))? else {
        return Err(ImportError::NotAPlaylist);
    };
    let items = match playlist::expand(path, format, cancel) {
        Ok(items) => items,
        // An empty playlist is still a playlist (S3.5).
        Err(PlaylistError::Empty) => Vec::new(),
        Err(PlaylistError::TooLarge) => {
            return Err(ImportError::Unreadable(Problem::PlaylistTooLarge));
        }
        Err(PlaylistError::Unreadable(kind)) => return Err(unreadable(kind)),
    };
    let missing = items.iter().filter_map(Media::as_path).filter(|path| !path.exists()).count();
    let title = path.file_stem().unwrap_or(path.as_os_str()).to_string_lossy().into_owned();
    Ok(Imported { title, items, missing })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::opening::scratch::scratch_dir;

    #[test]
    fn imports_every_entry_and_counts_the_missing_ones() {
        let dir = scratch_dir();
        fs::write(dir.join("a.mp3"), b"").unwrap();
        let text = "#EXTM3U\na.mp3\ngone.mp3\na.mp3\nhttp://radio/live\n\n#bad line\n";
        fs::write(dir.join("Road Trip.m3u"), text).unwrap();
        let imported = read(&dir.join("Road Trip.m3u"), &Cancel::default()).unwrap();
        assert_eq!(imported.title, "Road Trip");
        assert_eq!(imported.items.len(), 4);
        assert_eq!(imported.missing, 1);
        let message = imported.message("Road Trip (2)");
        assert_eq!(message, "Imported “Road Trip (2)” (4 items). 1 file is missing.");
    }

    #[test]
    fn an_empty_playlist_imports_empty() {
        let dir = scratch_dir();
        fs::write(dir.join("empty.pls"), "[playlist]\nNumberOfEntries=0\n").unwrap();
        let imported = read(&dir.join("empty.pls"), &Cancel::default()).unwrap();
        assert_eq!(imported.items, Vec::new());
    }

    #[test]
    fn other_files_are_refused() {
        let dir = scratch_dir();
        fs::write(dir.join("notes.txt"), "just some notes").unwrap();
        let refused = read(&dir.join("notes.txt"), &Cancel::default());
        assert_eq!(refused, Err(ImportError::NotAPlaylist));
        let gone = read(&dir.join("gone.m3u"), &Cancel::default());
        let expected = ImportError::Unreadable(Problem::Unreadable(io::ErrorKind::NotFound));
        assert_eq!(gone, Err(expected));
    }
}
