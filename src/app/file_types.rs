//! The file types and URL schemes the app registers with the OS (S0.6.3, S0.6.4).
//!
//! The packaging files under `packaging/` repeat this list in each platform's
//! format; the tests below keep them in sync with it.

/// A file extension and its MIME type.
pub struct FileType {
    /// The extension, without the dot.
    pub extension: &'static str,
    /// The MIME type desktop environments know it by.
    pub mime: &'static str,
}

const fn file(extension: &'static str, mime: &'static str) -> FileType {
    FileType { extension, mime }
}

/// Every file type the app offers to open.
pub const FILE_TYPES: &[FileType] = &[
    // Video.
    file("3gp", "video/3gpp"),
    file("avi", "video/x-msvideo"),
    file("flv", "video/x-flv"),
    file("m2ts", "video/mp2t"),
    file("m4v", "video/x-m4v"),
    file("mkv", "video/x-matroska"),
    file("mov", "video/quicktime"),
    file("mp4", "video/mp4"),
    file("mpeg", "video/mpeg"),
    file("mpg", "video/mpeg"),
    file("mts", "video/mp2t"),
    file("ogv", "video/ogg"),
    file("ts", "video/mp2t"),
    file("vob", "video/x-ms-vob"),
    file("webm", "video/webm"),
    file("wmv", "video/x-ms-wmv"),
    // Audio.
    file("aac", "audio/aac"),
    file("aiff", "audio/x-aiff"),
    file("ape", "audio/x-ape"),
    file("flac", "audio/flac"),
    file("m4a", "audio/mp4"),
    file("mka", "audio/x-matroska"),
    file("mp3", "audio/mpeg"),
    file("oga", "audio/ogg"),
    file("ogg", "audio/ogg"),
    file("opus", "audio/opus"),
    file("wav", "audio/x-wav"),
    file("wma", "audio/x-ms-wma"),
    // Playlists.
    file("m3u", "audio/x-mpegurl"),
    file("m3u8", "application/vnd.apple.mpegurl"),
    file("pls", "audio/x-scpls"),
    file("xspf", "application/xspf+xml"),
];

/// Streaming URL schemes the app registers for. Web schemes (`http`, `https`)
/// belong to the browser and are only accepted on the command line.
// Only the tests read it: they keep the packaging files in sync with it.
#[cfg(test)]
pub const URL_SCHEMES: &[&str] = &["mms", "rtmp", "rtsp"];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn packaging(path: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("packaging").join(path);
        std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
    }

    fn extensions() -> BTreeSet<&'static str> {
        FILE_TYPES.iter().map(|t| t.extension).collect()
    }

    /// Every `<string>…</string>` value after `key` in the plist fragment, up to the
    /// end of that array.
    fn plist_strings<'a>(plist: &'a str, key: &str) -> BTreeSet<&'a str> {
        let start = plist.find(&format!("<key>{key}</key>")).expect("key present");
        let rest = &plist[start..];
        let array = &rest[..rest.find("</array>").expect("array closes")];
        array.split("<string>").skip(1).filter_map(|s| s.split("</string>").next()).collect()
    }

    #[test]
    fn list_has_no_duplicates() {
        assert_eq!(extensions().len(), FILE_TYPES.len());
    }

    #[test]
    fn linux_desktop_entry_lists_every_mime_type() {
        let desktop = packaging("linux/redesigned-vlc.desktop");
        let line = desktop.lines().find_map(|l| l.strip_prefix("MimeType=")).expect("MimeType=");
        let listed: BTreeSet<&str> = line.split(';').filter(|m| !m.is_empty()).collect();
        let mut expected: BTreeSet<String> = FILE_TYPES.iter().map(|t| t.mime.into()).collect();
        expected.extend(URL_SCHEMES.iter().map(|s| format!("x-scheme-handler/{s}")));
        assert_eq!(listed, expected.iter().map(String::as_str).collect());
    }

    #[test]
    fn macos_plist_lists_every_extension_and_scheme() {
        let plist = packaging("macos/Info.plist.ext");
        assert_eq!(plist_strings(&plist, "CFBundleTypeExtensions"), extensions());
        let schemes: BTreeSet<&str> = URL_SCHEMES.iter().copied().collect();
        assert_eq!(plist_strings(&plist, "CFBundleURLSchemes"), schemes);
    }

    #[test]
    fn windows_registration_lists_every_extension() {
        let script = packaging("windows/register-file-types.ps1");
        let start = script.find("$Extensions = @(").expect("extension list");
        let list = &script[start..start + script[start..].find(')').expect("list closes")];
        let listed: BTreeSet<&str> = list.split('\'').skip(1).step_by(2).collect();
        assert_eq!(listed, extensions());
    }
}
