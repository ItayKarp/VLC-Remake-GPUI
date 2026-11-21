//! Reading playlist text and turning its entries into items to open.
//!
//! Playlists come from every era and OS: UTF-8 with or without a BOM, UTF-16 from Windows
//! Media Player, Latin-1 `.m3u` files, Windows paths in a playlist read on macOS.

use std::path::{Path, PathBuf};

use url::Url;

use crate::app::media::{Media, has_url_scheme};

/// Decodes a playlist file: by its BOM if it has one, as UTF-8 if it is valid, otherwise as
/// Latin-1 (old `.m3u` files), which can decode any byte.
pub fn decode(bytes: &[u8]) -> String {
    match bytes {
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        [0xFF, 0xFE, rest @ ..] => utf16(rest, u16::from_le_bytes),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, u16::from_be_bytes),
        _ => match std::str::from_utf8(bytes) {
            Ok(text) => text.to_owned(),
            Err(_) => bytes.iter().copied().map(char::from).collect(),
        },
    }
}

fn utf16(bytes: &[u8], unit: fn([u8; 2]) -> u16) -> String {
    let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|pair| unit(*pair)).collect();
    String::from_utf16_lossy(&units)
}

/// One entry of a playlist, before it is resolved against the playlist's location.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// A path (absolute, or relative to the playlist's folder) or a URL: M3U, PLS, CUE, ASX.
    Location(String),
    /// A URI reference, maybe relative to the playlist's own URI: XSPF.
    Uri(String),
}

/// The item `entry` names, for a playlist at `playlist`. `None` for a blank entry.
pub fn resolve(entry: &Entry, playlist: &Path) -> Option<Media> {
    match entry {
        Entry::Location(text) => location(text.trim(), playlist),
        Entry::Uri(text) => uri(text.trim(), playlist),
    }
}

fn location(text: &str, playlist: &Path) -> Option<Media> {
    if text.is_empty() {
        return None;
    }
    if has_url_scheme(text) {
        return Some(Media::from_url(text));
    }
    let path = native(text);
    if path.is_absolute() {
        return Some(Media::File(path));
    }
    let folder = playlist.parent().unwrap_or(Path::new(""));
    Some(Media::File(folder.join(path)))
}

/// An XSPF location: a URI, often relative (`music/a%20b.mp3`). Some writers put a plain path
/// there instead; that is read like any other location.
fn uri(text: &str, playlist: &Path) -> Option<Media> {
    if text.is_empty() || has_url_scheme(text) {
        return location(text, playlist);
    }
    let joined = Url::from_file_path(playlist).ok().and_then(|base| base.join(text).ok());
    match joined.map(|url| url.to_file_path()) {
        Some(Ok(path)) => Some(Media::File(path)),
        _ => location(text, playlist),
    }
}

/// A path written on another OS, in this OS's form. Windows reads `/` as a separator already;
/// elsewhere a `\` in a relative path written on Windows becomes `/`.
fn native(text: &str) -> PathBuf {
    if cfg!(windows) || text.contains('/') {
        return PathBuf::from(text);
    }
    PathBuf::from(text.replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playlist() -> PathBuf {
        std::env::temp_dir().join("lists").join("mix.m3u")
    }

    #[test]
    fn decodes_every_encoding() {
        assert_eq!(decode(b"\xEF\xBB\xBFcaf\xC3\xA9"), "café");
        assert_eq!(decode(b"caf\xE9"), "café");
        let utf16: Vec<u8> =
            [0xFF, 0xFE].into_iter().chain("é".encode_utf16().flat_map(u16::to_le_bytes)).collect();
        assert_eq!(decode(&utf16), "é");
    }

    #[test]
    fn relative_entries_resolve_against_the_playlist_folder() {
        let list = playlist();
        let folder = list.parent().unwrap().to_path_buf();
        let resolved = resolve(&Entry::Location(" a.mp3 ".into()), &list);
        assert_eq!(resolved, Some(Media::File(folder.join("a.mp3"))));
        let resolved = resolve(&Entry::Uri("sub/a%20b.mp3".into()), &list);
        assert_eq!(resolved, Some(Media::File(folder.join("sub").join("a b.mp3"))));
        assert_eq!(resolve(&Entry::Location("  ".into()), &list), None);
    }

    #[test]
    fn urls_and_absolute_paths_stay_as_they_are() {
        let list = playlist();
        let url = resolve(&Entry::Location("http://radio/stream".into()), &list);
        assert_eq!(url, Some(Media::Url("http://radio/stream".into())));
        let absolute = std::env::temp_dir().join("x.mkv");
        let resolved = resolve(&Entry::Location(absolute.to_string_lossy().into()), &list);
        assert_eq!(resolved, Some(Media::File(absolute)));
    }

    #[cfg(unix)]
    #[test]
    fn windows_relative_paths_work_on_unix() {
        let list = playlist();
        let resolved = resolve(&Entry::Location(r"Album\01 Song.mp3".into()), &list);
        let expected = list.parent().unwrap().join("Album").join("01 Song.mp3");
        assert_eq!(resolved, Some(Media::File(expected)));
    }
}
