//! What a library folder lists (S4.6.1): its subfolders, and the media of the section's kind as
//! cards, each in natural order (`E2` before `E10`).
//!
//! The disk says what is there, so a folder the indexer hasn't reached yet, or one outside every
//! media folder, still lists its media. The library adds what it knows: each file's media (for
//! thumbnails and keeping a selection across a rename), its kind and how far it was watched.
//!
//! Each section shows its own kind: Videos lists video, Music lists audio. A file not probed yet
//! goes by its extension, and one whose extension says neither shows in both.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use super::path_key::Case;
use super::recognize;
use super::watch_state::WatchState;
use crate::app::file_types::FILE_TYPES;
use crate::db::files::{self, Listed};
use crate::db::folders;
use crate::db::media::MediaId;
use crate::db::metadata::{MediaKind, Metadata};
use crate::history::policy::WatchedRule;
use crate::opening::scan::natural_cmp;

/// One media file, as its card shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    /// What the card says: the file name in a folder, the title in Music's categories.
    pub label: String,
    /// Where it is.
    pub path: PathBuf,
    /// Its library media, once the indexer identified it.
    pub media: Option<MediaId>,
    /// Video or audio, as far as is known.
    pub kind: MediaKind,
    /// How far it was watched.
    pub watch: WatchState,
    /// Its tags, as far as the library read them, for sorting (S4.7).
    pub tags: Metadata,
    /// When its media joined the library, in milliseconds since the epoch, if known.
    pub added_ms: Option<i64>,
}

/// What a folder holds on the disk: its subfolders and its media files, unsorted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OnDisk {
    /// The subfolders.
    pub folders: Vec<PathBuf>,
    /// The media files.
    pub files: Vec<PathBuf>,
}

/// Reads `folder`'s subfolders and media files; hidden ones only when the library shows them.
/// Blocks on the disk.
pub fn read_disk(folder: &Path, show_hidden: bool) -> io::Result<OnDisk> {
    let mut on_disk = OnDisk::default();
    for entry in std::fs::read_dir(folder)?.filter_map(Result::ok) {
        if !show_hidden && entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        // `metadata` follows symlinks, so a link to a folder lists as that folder.
        let Ok(metadata) = std::fs::metadata(entry.path()) else { continue };
        if metadata.is_dir() {
            on_disk.folders.push(entry.path());
        } else if metadata.is_file() && recognize::is_media(&entry.path(), false) {
            on_disk.files.push(entry.path());
        }
    }
    Ok(on_disk)
}

/// What the library knows of the files in `folder`; nothing when it doesn't index the folder.
pub fn read_library(connection: &Connection, folder: &Path) -> rusqlite::Result<Vec<Listed>> {
    // The exact key first: a case-insensitive root stores lowercase keys, which only an
    // already-lowercase path matches exactly, and that is the same folder anyway.
    for case in [Case::Sensitive, Case::Insensitive] {
        if let Some(id) = folders::find(connection, folder, case)? {
            return files::listed(connection, id);
        }
    }
    Ok(Vec::new())
}

/// The cards for `files` that are `wanted` (video or audio), in natural order of their names,
/// with what the library knows (`known`) and `rule` deciding what is watched.
pub fn cards(
    files: Vec<PathBuf>,
    known: Vec<Listed>,
    wanted: MediaKind,
    rule: WatchedRule,
) -> Vec<Card> {
    let known: HashMap<PathBuf, Listed> =
        known.into_iter().map(|listed| (listed.name.clone(), listed)).collect();
    let mut cards: Vec<Card> = files
        .into_iter()
        .map(|path| card(path, &known, rule))
        .filter(|card| card.kind == wanted || card.kind == MediaKind::Unknown)
        .collect();
    cards.sort_by(|a, b| natural_cmp(&a.label, &b.label));
    cards
}

/// `folders` in natural order of their names.
pub fn sort_folders(folders: &mut [PathBuf]) {
    folders.sort_by(|a, b| natural_cmp(&file_name(a), &file_name(b)));
}

fn card(path: PathBuf, known: &HashMap<PathBuf, Listed>, rule: WatchedRule) -> Card {
    let listed = path.file_name().and_then(|name| known.get(Path::new(name)));
    let tags = listed.map(|listed| listed.metadata.clone()).unwrap_or_default();
    let kind = if tags.kind == MediaKind::Unknown { kind_by_extension(&path) } else { tags.kind };
    let watch = listed.map_or(WatchState::Unwatched, |listed| WatchState::of(&listed.saved, rule));
    Card {
        label: file_name(&path),
        media: listed.and_then(|listed| listed.media),
        path,
        kind,
        watch,
        tags,
        added_ms: listed.and_then(|listed| listed.added_ms),
    }
}

/// Video or audio, as the file's extension says; unknown for one that says neither.
pub fn kind_by_extension(path: &Path) -> MediaKind {
    let Some(extension) = path.extension().and_then(|e| e.to_str()) else {
        return MediaKind::Unknown;
    };
    let file_type = FILE_TYPES.iter().find(|t| t.extension.eq_ignore_ascii_case(extension));
    match file_type.map(|t| t.mime) {
        Some(mime) if mime.starts_with("video/") => MediaKind::Video,
        Some(mime) if mime.starts_with("audio/") => MediaKind::Audio,
        _ => MediaKind::Unknown,
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().unwrap_or(path.as_os_str()).to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests;
