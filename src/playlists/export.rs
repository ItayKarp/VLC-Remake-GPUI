//! Exporting a saved playlist to a file (S3.6.1): `.m3u8` (UTF-8, `#EXTINF` with duration and
//! title) or `.xspf`, with paths relative to the exported file or absolute.
//!
//! [`render`] builds the file's text and [`write`] puts it on disk atomically, so a read-only
//! folder or a full disk leaves the old file (or none), never half a playlist.

mod m3u8;
mod xspf;

use std::fs::{self, File};
use std::io::{self, Write as _};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use crate::app::media::Media;

/// The file formats a playlist exports to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Format {
    /// Extended M3U in UTF-8.
    #[default]
    M3u8,
    /// XML Shareable Playlist Format.
    Xspf,
}

impl Format {
    /// Both formats, in the order the export dialog offers them.
    pub const ALL: [Format; 2] = [Format::M3u8, Format::Xspf];

    /// The file extension, without the dot.
    pub fn extension(self) -> &'static str {
        match self {
            Format::M3u8 => "m3u8",
            Format::Xspf => "xspf",
        }
    }

    /// The format's name in the export dialog.
    pub fn label(self) -> &'static str {
        match self {
            Format::M3u8 => "M3U8",
            Format::Xspf => "XSPF",
        }
    }

    /// The format the file at `path` should have: the one its extension names, so a name typed
    /// in the save dialog wins, else `chosen`.
    pub fn for_path(path: &Path, chosen: Format) -> Format {
        let extension = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
        match extension.as_deref() {
            Some("xspf") => Format::Xspf,
            Some("m3u8" | "m3u") => Format::M3u8,
            _ => chosen,
        }
    }
}

/// How local files are written into the exported playlist.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PathStyle {
    /// Relative to the exported file's folder, so the playlist moves with its media. A file
    /// that has no relative path (another Windows drive) is written absolute.
    #[default]
    Relative,
    /// Absolute, so the playlist can move without its media.
    Absolute,
}

/// One item to export.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// What it plays.
    pub media: Media,
    /// Its length, when the history knows it.
    pub duration: Option<Duration>,
}

impl Item {
    /// The title written for it: the file name without its extension, or the URL.
    fn title(&self) -> String {
        match &self.media {
            Media::File(path) => {
                path.file_stem().unwrap_or(path.as_os_str()).to_string_lossy().into_owned()
            }
            Media::Url(url) => url.clone(),
        }
    }
}

/// What a playlist export writes.
#[derive(Clone, Copy, Debug)]
pub struct Export<'a> {
    /// The playlist's title.
    pub title: &'a str,
    /// Its items, in order.
    pub items: &'a [Item],
    /// How local files are written.
    pub paths: PathStyle,
}

/// The text of `export` in `format`, for a file at `destination`.
pub fn render(export: &Export, format: Format, destination: &Path) -> String {
    let folder = destination.parent().unwrap_or(Path::new(""));
    match format {
        Format::M3u8 => m3u8::render(export, folder),
        Format::Xspf => xspf::render(export, folder),
    }
}

/// `path` relative to `folder`, when both are absolute and on the same root.
fn relative_path(path: &Path, folder: &Path) -> Option<PathBuf> {
    if !path.is_absolute() || !folder.is_absolute() {
        return None;
    }
    let mut path_parts = path.components().peekable();
    let mut folder_parts = folder.components().peekable();
    if path_parts.peek() != folder_parts.peek() {
        return None;
    }
    while path_parts.peek().is_some() && path_parts.peek() == folder_parts.peek() {
        path_parts.next();
        folder_parts.next();
    }
    let ups = folder_parts.map(|_| Component::ParentDir);
    Some(ups.chain(path_parts).collect())
}

/// Atomically replaces `path` with `contents`: written beside it, flushed, then renamed over it.
/// On failure the temporary file is removed and `path` is untouched.
pub fn write(path: &Path, contents: &str) -> io::Result<()> {
    let name = path.file_name().ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
    let temporary =
        path.with_file_name(format!(".{}.{}.tmp", name.to_string_lossy(), std::process::id()));
    let written =
        write_synced(&temporary, contents.as_bytes()).and_then(|()| fs::rename(&temporary, path));
    if written.is_err() {
        // It may not exist; the write's own error is the one to report.
        let _ = fs::remove_file(&temporary);
    }
    written
}

fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// What the user is told when an export couldn't be written.
pub fn error_message(error: &io::Error) -> &'static str {
    match error.kind() {
        io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => {
            "Couldn't export the playlist: that folder can't be written to."
        }
        io::ErrorKind::StorageFull | io::ErrorKind::QuotaExceeded => {
            "Couldn't export the playlist: the disk is full."
        }
        _ => "Couldn't export the playlist.",
    }
}

#[cfg(test)]
mod tests;
