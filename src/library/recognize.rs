//! Which files the library indexes (S4.2.2): media by its extension, and by a look at its first
//! bytes when the extension says nothing (none, or one nobody uses for documents or pictures).
//!
//! Files whose extension names something else (pictures, documents, archives, downloads in
//! progress) are never opened, so a scan of a folder full of photos reads no file. Online-only
//! files are never opened either: reading them would download them.

use std::path::Path;

use crate::opening::kind::{self, Kind};

/// Extensions of files that are certainly not media, lowercase: skipped without a look inside.
/// Downloads in progress (`.part`) are among them; the finished file gets its real name.
const OTHER_EXTENSIONS: &[&str] = &[
    "7z",
    "ai",
    "app",
    "avif",
    "bak",
    "bat",
    "bmp",
    "bz2",
    "cfg",
    "crdownload",
    "css",
    "csv",
    "db",
    "dll",
    "dmg",
    "doc",
    "docx",
    "download",
    "epub",
    "exe",
    "gif",
    "gz",
    "heic",
    "heif",
    "htm",
    "html",
    "ico",
    "ini",
    "iso",
    "jpeg",
    "jpg",
    "js",
    "json",
    "lnk",
    "log",
    "md",
    "mobi",
    "msi",
    "nfo",
    "opdownload",
    "part",
    "pdf",
    "pkg",
    "png",
    "ppt",
    "pptx",
    "psd",
    "py",
    "rar",
    "rtf",
    "sfv",
    "sh",
    "sqlite",
    "svg",
    "tar",
    "tif",
    "tiff",
    "tmp",
    "torrent",
    "txt",
    "url",
    "webp",
    "xls",
    "xlsx",
    "xml",
    "xz",
    "zip",
];

/// Whether the regular file at `path` is media the library should index. `cloud` says it is an
/// online-only placeholder, whose content mustn't be read.
pub fn is_media(path: &Path, cloud: bool) -> bool {
    if let Some(kind) = kind::by_extension(path) {
        return kind == Kind::Media;
    }
    if cloud || has_other_extension(path) {
        return false;
    }
    matches!(kind::classify(path), Ok(Kind::Media))
}

fn has_other_extension(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|e| e.to_str()) else { return false };
    OTHER_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::opening::scratch::scratch_dir;

    const MATROSKA: &[u8] = b"\x1A\x45\xDF\xA3rest";

    #[test]
    fn known_extensions_are_trusted_without_reading() {
        let dir = scratch_dir();
        // Neither file exists: the extension alone decides.
        assert!(is_media(&dir.join("Movie.MKV"), false));
        assert!(!is_media(&dir.join("notes.srt"), false));
        assert!(!is_media(&dir.join("cover.JPG"), false));
    }

    #[test]
    fn files_without_a_telling_extension_are_looked_into() {
        let dir = scratch_dir();
        for name in ["recording", "clip.bin"] {
            fs::write(dir.join(name), MATROSKA).expect("write");
            assert!(is_media(&dir.join(name), false), "{name}");
        }
        fs::write(dir.join("readme"), b"plain text").expect("write");
        assert!(!is_media(&dir.join("readme"), false));
    }

    #[test]
    fn online_only_files_and_pictures_are_never_read() {
        let dir = scratch_dir();
        fs::write(dir.join("photo.png"), MATROSKA).expect("write");
        fs::write(dir.join("recording"), MATROSKA).expect("write");
        assert!(!is_media(&dir.join("photo.png"), false));
        assert!(!is_media(&dir.join("recording"), true));
        assert!(is_media(&dir.join("in-the-cloud.mp4"), true));
    }

    #[test]
    fn the_lists_dont_overlap() {
        for extension in OTHER_EXTENSIONS {
            assert_eq!(*extension, extension.to_ascii_lowercase());
            let name = format!("x.{extension}");
            assert_eq!(kind::by_extension(Path::new(&name)), None, "{name}");
        }
    }
}
