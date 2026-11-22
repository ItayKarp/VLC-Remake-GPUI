//! Subtitle files that load with a video (S6.1): the ones beside it and in a `Subs` or
//! `Subtitles` folder next to it, picked by "File name matching", checked before they load, and
//! with the language and forced flag their names give.
//!
//! The app finds them itself rather than leaving it to libVLC, so it knows which track came
//! from which file: the track popover lists them by name, and their languages take part in
//! picking a subtitle by the preferred languages.
//!
//! - [`matching`]: the File name matching levels.
//! - [`format`]: the formats, a size limit and a look at each file's start.
//! - [`naming`]: the language and forced flag in a name.

mod format;
mod matching;
mod naming;
#[cfg(test)]
mod tests;

use std::fs;
use std::path::{Path, PathBuf};

use format::Format;

use super::language::Language;
use super::subtitle_style::FileMatching;

/// Folders beside a video that hold its subtitles, compared ignoring case. In one of them, a
/// folder named after the video holds only its subtitles (`Subs/Movie/2_English.srt`).
const SUBTITLE_FOLDERS: [&str; 4] = ["subs", "subtitles", "sub", "subtitle"];
/// The most files that load with one video, so "Any subtitle in the folder" in a folder of a
/// whole series doesn't add hundreds of tracks.
const MAX_FILES: usize = 32;

/// A subtitle file found for a video.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sidecar {
    /// Where the file is.
    pub path: PathBuf,
    /// The language its name gives (`Movie.en.srt`), if any.
    pub language: Option<Language>,
    /// Its name marks it forced (`Movie.en.forced.srt`).
    pub forced: bool,
}

/// Whether the file at `path` holds text subtitles, by its extension: what an encoding applies
/// to ([`super::subtitle_text`]). Picture formats (PGS, VobSub) don't.
pub fn is_text(path: &Path) -> bool {
    format_of(path).is_some_and(Format::is_text)
}

/// How a folder's files are matched to the video.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scope {
    /// By name, at the chosen File name matching level.
    ByName,
    /// All of them: the folder is named after the video.
    All,
}

/// The subtitle files that go with the video at `media` at `level`, beside it first, each
/// folder's with the video's exact name first, then in name order. Reads folders and the start of each matching file, so it belongs
/// off the UI thread; a folder that can't be read adds nothing.
pub fn find(media: &Path, level: FileMatching) -> Vec<Sidecar> {
    let (Some(folder), Some(title)) = (media.parent(), media.file_stem()) else {
        return Vec::new();
    };
    let title = title.to_string_lossy();
    let mut found = Vec::new();
    for (folder, scope) in folders(folder, &title) {
        let wanted = |name: &str| scope == Scope::All || matching::matches(&title, name, level);
        let mut paths = subtitle_files(&folder);
        // The exact name first: when nothing else decides, it is the one that shows.
        paths.sort_by_key(|path| !stem(path).eq_ignore_ascii_case(&title));
        for path in paths {
            let name = stem(&path);
            if path != media && wanted(&name) && checks_out(&path) {
                let naming = naming::read(&name, &title);
                found.push(Sidecar { path, language: naming.language, forced: naming.forced });
            }
        }
    }
    if found.len() > MAX_FILES {
        tracing::info!("subtitles: {} files match; loading the first {MAX_FILES}", found.len());
        found.truncate(MAX_FILES);
    }
    found
}

/// The folders to look in: `folder`, then its subtitle folders, each followed by a folder in it
/// named after `title`.
fn folders(folder: &Path, title: &str) -> Vec<(PathBuf, Scope)> {
    let mut folders = vec![(folder.to_path_buf(), Scope::ByName)];
    for subtitles in subfolders(folder, |name| {
        SUBTITLE_FOLDERS.iter().any(|known| name.eq_ignore_ascii_case(known))
    }) {
        let named = subfolders(&subtitles, |name| name.eq_ignore_ascii_case(title));
        folders.push((subtitles, Scope::ByName));
        folders.extend(named.into_iter().map(|folder| (folder, Scope::All)));
    }
    folders
}

/// The folders in `folder` whose names `wanted` takes, in name order.
fn subfolders(folder: &Path, wanted: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = entries(folder)
        .filter(|path| {
            path.is_dir() && path.file_name().is_some_and(|name| wanted(&name.to_string_lossy()))
        })
        .collect();
    found.sort();
    found
}

/// The files in `folder` with a subtitle extension, in name order: no hidden files (macOS's
/// `._Movie.srt`), and no `.sub` holding the pictures of a VobSub `.idx` beside it.
fn subtitle_files(folder: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = entries(folder)
        .filter(|path| {
            let hidden =
                path.file_name().is_none_or(|name| name.to_string_lossy().starts_with('.'));
            !hidden && format_of(path).is_some() && !is_vobsub_pictures(path) && path.is_file()
        })
        .collect();
    found.sort();
    found
}

/// Whether the file at `path` is what its extension says, logging the ones that aren't.
fn checks_out(path: &Path) -> bool {
    let fits = format_of(path).is_some_and(|format| match format {
        Format::VobSub => sibling(path, "sub").is_some() && format.checks_out(path),
        _ => format.checks_out(path),
    });
    if !fits {
        tracing::info!(
            "subtitles: skipping {}: too big, or not subtitles",
            crate::logging::path(path)
        );
    }
    fits
}

fn is_vobsub_pictures(path: &Path) -> bool {
    format_of(path) == Some(Format::TextSub) && sibling(path, "idx").is_some()
}

/// The file beside `path` with the same name and `extension`, in lower or upper case.
fn sibling(path: &Path, extension: &str) -> Option<PathBuf> {
    [extension.to_owned(), extension.to_ascii_uppercase()]
        .into_iter()
        .map(|extension| path.with_extension(extension))
        .find(|other| other.is_file())
}

fn format_of(path: &Path) -> Option<Format> {
    path.extension().and_then(|extension| Format::from_extension(&extension.to_string_lossy()))
}

fn stem(path: &Path) -> String {
    path.file_stem().map_or_default(|stem| stem.to_string_lossy().into_owned())
}

fn entries(folder: &Path) -> impl Iterator<Item = PathBuf> {
    fs::read_dir(folder).into_iter().flatten().flatten().map(|entry| entry.path())
}
