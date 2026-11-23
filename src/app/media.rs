//! What the app is asked to open: a local file or folder, a network URL (S0.6.2), or a disc or
//! capture device (S8.9).
//!
//! Arguments are normalized in the process that received them, so relative paths
//! resolve against the *launching* process's working directory even when the request
//! is forwarded to an instance that's already running (S0.6.1).

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use url::Url;

pub use self::device::{Device, DeviceKind};
use crate::settings::stored_path;

pub mod device;

/// One item to open.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Media {
    /// A local file or folder, as an absolute path.
    File(PathBuf),
    /// A network stream or any other URL that isn't a local file (`https://…`,
    /// `rtsp://…`).
    Url(String),
    /// A disc or capture device (S8.9).
    Device(Device),
}

impl Media {
    /// Turns one command-line argument into an item. `file://` URLs become paths
    /// (percent-decoded, `file://server/share` becomes a UNC path on Windows), other
    /// URLs stay URLs, and relative paths are made absolute against this process's
    /// working directory. Returns `None` for an empty argument.
    pub fn from_arg(arg: &OsStr) -> Option<Self> {
        if arg.is_empty() {
            return None;
        }
        if let Some(text) = arg.to_str().filter(|text| has_url_scheme(text)) {
            return Some(Self::from_url(text));
        }
        Some(Media::File(absolute(Path::new(arg))))
    }

    /// Turns a URL (from the command line or the OS, e.g. macOS Finder's "Open
    /// With") into an item. A `file://` URL that can't be a local path is kept as a
    /// URL, so nothing the user asked for is dropped silently.
    pub fn from_url(text: &str) -> Self {
        if let Some(device) = Device::new(text) {
            return Media::Device(device);
        }
        match Url::parse(text) {
            Ok(url) if url.scheme() == "file" => {
                url.to_file_path().map_or_else(|()| Media::Url(text.to_owned()), Media::File)
            }
            _ => Media::Url(text.to_owned()),
        }
    }

    /// The local path, if this is a file.
    pub fn as_path(&self) -> Option<&Path> {
        match self {
            Media::File(path) => Some(path),
            Media::Url(_) | Media::Device(_) => None,
        }
    }

    /// The text settings and the session file keep this as (S1.3.5, S3.4): a URL as written,
    /// a path in [`stored_path`]'s lossless form, a device as [`Device::to_stored`] says. A
    /// stored path never looks like a URL: it is absolute, and an encoded one starts with
    /// `os-path:`, which has no `//`.
    pub fn to_stored(&self) -> String {
        match self {
            Media::File(path) => stored_path::encode(path),
            Media::Url(url) => url.clone(),
            Media::Device(device) => device.to_stored(),
        }
    }

    /// The inverse of [`Self::to_stored`]; `None` for text that is neither.
    pub fn from_stored(text: &str) -> Option<Self> {
        if let Some(device) = Device::from_stored(text) {
            return Some(Media::Device(device));
        }
        if has_url_scheme(text) {
            return Some(Media::Url(text.to_owned()));
        }
        stored_path::decode(text).map(Media::File)
    }

    /// A short name for the window title and lists: the file name, the URL with any password
    /// in it masked, or the device's title or kind.
    pub fn display_name(&self) -> String {
        match self {
            Media::File(path) => {
                path.file_name().unwrap_or(path.as_os_str()).to_string_lossy().into_owned()
            }
            Media::Url(url) => crate::network::links::masked(url),
            Media::Device(device) => device.display_name(),
        }
    }

    /// The device, if this is a disc or capture device.
    pub fn as_device(&self) -> Option<&Device> {
        match self {
            Media::Device(device) => Some(device),
            Media::File(_) | Media::Url(_) => None,
        }
    }
}

/// Whether `text` starts with `scheme://`. A scheme is at least two characters, so
/// `C://video.mkv`-style drive paths are never mistaken for URLs.
pub(crate) fn has_url_scheme(text: &str) -> bool {
    let Some((scheme, _)) = text.split_once("://") else {
        return false;
    };
    let mut chars = scheme.chars();
    scheme.len() >= 2
        && chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// `path` made absolute against the working directory, without touching the file
/// system: UNC paths and `\\?\` long paths are kept as they are, and symlinks aren't
/// resolved (the user opened the link, so that's the name to show).
fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Builds the item for an argument that's already known to be a path, e.g. from a
/// file dialog or a drop.
impl From<PathBuf> for Media {
    fn from(path: PathBuf) -> Self {
        Media::File(path)
    }
}

/// The bytes of an `OsStr` on this platform, for the forwarding wire format.
#[cfg(unix)]
pub(super) fn os_to_bytes(text: &OsStr) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    text.as_bytes().to_vec()
}

/// The bytes of an `OsStr` on this platform, for the forwarding wire format: UTF-16
/// code units, little-endian, so unpaired surrogates survive.
#[cfg(windows)]
pub(super) fn os_to_bytes(text: &OsStr) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    text.encode_wide().flat_map(u16::to_le_bytes).collect()
}

/// The inverse of [`os_to_bytes`].
#[cfg(unix)]
#[allow(clippy::unnecessary_wraps, reason = "same signature as the Windows one, which can fail")]
pub(super) fn os_from_bytes(bytes: &[u8]) -> Option<OsString> {
    use std::os::unix::ffi::OsStringExt;
    Some(OsString::from_vec(bytes.to_vec()))
}

/// The inverse of [`os_to_bytes`]. Fails on an odd number of bytes.
#[cfg(windows)]
pub(super) fn os_from_bytes(bytes: &[u8]) -> Option<OsString> {
    use std::os::windows::ffi::OsStringExt;
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let wide: Vec<u16> =
        bytes.as_chunks::<2>().0.iter().map(|pair| u16::from_le_bytes(*pair)).collect();
    Some(OsString::from_wide(&wide))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arg(text: &str) -> Media {
        Media::from_arg(OsStr::new(text)).expect("non-empty argument")
    }

    #[test]
    fn urls_stay_urls() {
        assert_eq!(
            arg("https://example.com/a b.mp4"),
            Media::Url("https://example.com/a b.mp4".into())
        );
        assert_eq!(arg("rtsp://cam.local/live"), Media::Url("rtsp://cam.local/live".into()));
        assert!(matches!(arg("dvd:///dev/rdisk4"), Media::Device(_)));
        assert!(Media::from_arg(OsStr::new("")).is_none());
    }

    #[test]
    fn relative_paths_become_absolute() {
        let Media::File(path) = arg("movie.mkv") else { panic!("expected a file") };
        assert!(path.is_absolute());
        assert_eq!(path, std::env::current_dir().expect("cwd").join("movie.mkv"));
    }

    #[test]
    fn drive_paths_are_not_urls() {
        assert!(!has_url_scheme("C://video.mkv"));
        assert!(!has_url_scheme("no scheme here"));
        assert!(!has_url_scheme("1http://x"));
        assert!(has_url_scheme("svn+ssh://host/x"));
    }

    #[cfg(windows)]
    #[test]
    fn file_urls_become_windows_paths() {
        assert_eq!(
            arg("file:///C:/Videos/caf%C3%A9%20%231.mkv"),
            Media::File(r"C:\Videos\café #1.mkv".into())
        );
        assert_eq!(arg("file://server/share/a.mkv"), Media::File(r"\\server\share\a.mkv".into()));
        assert_eq!(arg(r"\\server\share\a.mkv"), Media::File(r"\\server\share\a.mkv".into()));
        assert_eq!(arg(r"\\?\C:\long\a.mkv"), Media::File(r"\\?\C:\long\a.mkv".into()));
    }

    #[cfg(unix)]
    #[test]
    fn file_urls_become_unix_paths() {
        assert_eq!(
            arg("file:///home/me/caf%C3%A9%20%231.mkv"),
            Media::File("/home/me/café #1.mkv".into())
        );
    }

    #[test]
    fn os_strings_round_trip() {
        let text = OsString::from("Ünïcode ✓ \"quoted\"");
        assert_eq!(os_from_bytes(&os_to_bytes(&text)), Some(text));
    }

    #[test]
    fn stored_paths_and_urls_round_trip() {
        let file = Media::File(std::env::temp_dir().join("café #1.mkv"));
        for media in [file, Media::Url("rtsp://cam/1".into())] {
            assert_eq!(Media::from_stored(&media.to_stored()), Some(media));
        }
        let plain = Media::from_stored("/abs/path.mkv");
        assert_eq!(plain, Some(Media::File(PathBuf::from("/abs/path.mkv"))));
    }

    #[test]
    fn display_names() {
        assert_eq!(Media::File(PathBuf::from("dir").join("a.mkv")).display_name(), "a.mkv");
        assert_eq!(Media::Url("https://x/y".into()).display_name(), "https://x/y");
    }
}
