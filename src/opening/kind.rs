//! What a local file is (S1.3.6): media, a playlist, a subtitle, or something else.
//!
//! A file whose extension is a known media, playlist or subtitle type is taken at its word,
//! matched case-insensitively (`.MKV`), so scanning a large folder doesn't read every file.
//! Every other file, including one with no extension or a misleading one (a video saved as
//! `.txt`), is identified by its first bytes ([`super::sniff`]).

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use super::sniff;

/// How many bytes are read to identify a file by its content.
const HEAD_SIZE: u64 = 4096;

/// What a file holds, as far as opening it is concerned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Audio or video for the engine.
    Media,
    /// A list of other items, expanded instead of played (S1.3.4).
    Playlist(PlaylistFormat),
    /// A subtitle file, attached to a video rather than played.
    Subtitle,
    /// Nothing the app plays.
    Other,
}

/// The playlist formats the app reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaylistFormat {
    /// `.m3u` and `.m3u8`: one entry per line. An HLS stream uses the same syntax and is played,
    /// not expanded.
    M3u,
    /// `.pls`: an INI file with `File1=…` keys.
    Pls,
    /// `.xspf`: XML with `<track><location>` entries.
    Xspf,
    /// `.cue`: a CD sheet whose `FILE` lines name the audio files.
    Cue,
    /// `.asx`: Windows Media XML with `<entry><ref href>` entries.
    Asx,
}

/// Audio and video extensions, lowercase. Wider than the types registered with the OS
/// (`app::file_types`): anything libVLC plays that people keep in their media folders.
const MEDIA_EXTENSIONS: &[&str] = &[
    "3g2", "3gp", "aac", "ac3", "aif", "aifc", "aiff", "amr", "ape", "asf", "au", "avi", "caf",
    "dts", "dv", "divx", "eac3", "f4v", "flac", "flv", "m1v", "m2t", "m2ts", "m2v", "m4a", "m4b",
    "m4p", "m4v", "mid", "midi", "mka", "mkv", "mod", "mov", "mp2", "mp3", "mp4", "mpc", "mpe",
    "mpeg", "mpg", "mpv", "mts", "mxf", "nsv", "nut", "oga", "ogg", "ogm", "ogv", "ogx", "opus",
    "qt", "ra", "ram", "rm", "rmvb", "spx", "tod", "ts", "tta", "vob", "voc", "vro", "w64", "wav",
    "weba", "webm", "wma", "wmv", "wtv", "wv", "xm",
];

/// Subtitle extensions, lowercase.
const SUBTITLE_EXTENSIONS: &[&str] =
    &["ass", "idx", "jss", "lrc", "sbv", "smi", "srt", "ssa", "sub", "sup", "ttml", "usf", "vtt"];

/// The kind `path`'s extension names, if it is a known one. Case doesn't matter.
pub fn by_extension(path: &Path) -> Option<Kind> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    let extension = extension.as_str();
    if MEDIA_EXTENSIONS.contains(&extension) {
        return Some(Kind::Media);
    }
    if SUBTITLE_EXTENSIONS.contains(&extension) {
        return Some(Kind::Subtitle);
    }
    let format = match extension {
        "m3u" | "m3u8" => PlaylistFormat::M3u,
        "pls" => PlaylistFormat::Pls,
        "xspf" => PlaylistFormat::Xspf,
        "cue" => PlaylistFormat::Cue,
        "asx" | "wax" | "wvx" => PlaylistFormat::Asx,
        _ => return None,
    };
    Some(Kind::Playlist(format))
}

/// What the regular file at `path` is: by its extension when that is a known one, otherwise by
/// its first bytes. Reads the file, so call it off the UI thread.
pub fn classify(path: &Path) -> io::Result<Kind> {
    if let Some(kind) = by_extension(path) {
        return Ok(kind);
    }
    let mut head = Vec::new();
    File::open(path)?.take(HEAD_SIZE).read_to_end(&mut head)?;
    Ok(sniff::kind(&head).unwrap_or(Kind::Other))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_match_case_insensitively() {
        assert_eq!(by_extension(Path::new("A.MKV")), Some(Kind::Media));
        assert_eq!(by_extension(Path::new("b.Srt")), Some(Kind::Subtitle));
        assert_eq!(by_extension(Path::new("c.M3U8")), Some(Kind::Playlist(PlaylistFormat::M3u)));
        assert_eq!(by_extension(Path::new("d.pdf")), None);
        assert_eq!(by_extension(Path::new("no_extension")), None);
    }

    #[test]
    fn every_registered_file_type_is_recognized() {
        for file_type in crate::app::file_types::FILE_TYPES {
            let name = format!("x.{}", file_type.extension);
            assert!(by_extension(Path::new(&name)).is_some(), "{name}");
        }
    }

    #[test]
    fn lists_are_lowercase_and_disjoint() {
        for extension in MEDIA_EXTENSIONS.iter().chain(SUBTITLE_EXTENSIONS) {
            assert_eq!(*extension, extension.to_ascii_lowercase());
        }
        assert!(MEDIA_EXTENSIONS.iter().all(|e| !SUBTITLE_EXTENSIONS.contains(e)));
    }
}
