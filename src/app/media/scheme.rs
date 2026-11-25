//! Which links VLC opens (S9.7.2): only schemes libVLC streams media from. `javascript:`,
//! `data:`, `vbscript:` and every other scheme are refused before they reach libVLC, which has
//! access modules (`fd://`, `imem://`, `concat://`, …) no link from a playlist or a web page
//! should reach.
//!
//! Discs and capture devices have schemes of their own ([`super::device`]); they are opened
//! through VLC's own dialogs, and a playlist file may name a disc but never a camera or the
//! screen.

use super::{DeviceKind, Media};

/// The schemes a stream may use.
const MEDIA_SCHEMES: &[&str] = &[
    "http", "https", "rtsp", "rtsps", "rtmp", "rtmps", "mms", "mmsh", "udp", "rtp", "smb", "ftp",
    "ftps", "sftp", "file",
];

/// Whether `link` is `scheme://…` with a scheme a stream may use, and something after it.
pub fn is_media_link(link: &str) -> bool {
    let Some((scheme, rest)) = link.split_once("://") else { return false };
    !rest.is_empty() && MEDIA_SCHEMES.iter().any(|known| known.eq_ignore_ascii_case(scheme))
}

impl Media {
    /// Whether the player may open this: any file or device, and only links with a media
    /// scheme.
    pub fn is_openable(&self) -> bool {
        match self {
            Media::File(_) | Media::Device(_) => true,
            Media::Url(url) => is_media_link(url),
        }
    }

    /// Whether a playlist file may list this: what the player opens, but no camera or screen
    /// capture, which a downloaded playlist must not be able to start.
    pub fn may_be_listed(&self) -> bool {
        let capture = self
            .as_device()
            .is_some_and(|device| matches!(device.kind(), DeviceKind::Camera | DeviceKind::Screen));
        self.is_openable() && !capture
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_media_schemes_open() {
        for link in ["https://a/b.m3u8", "HTTP://X/y", "rtsp://cam/1", "udp://@:1234", "smb://s/a"]
        {
            assert!(Media::from_url(link).is_openable(), "{link}");
        }
        for link in [
            "javascript://%0aalert(1)",
            "data://text/plain,x",
            "vbscript://x",
            "fd://0",
            "imem://",
            "concat://a,b",
            "vlc://quit",
            "https://",
        ] {
            assert!(!Media::from_url(link).is_openable(), "{link}");
        }
    }

    #[test]
    fn playlists_name_discs_but_never_capture() {
        assert!(Media::from_url("dvd:///dev/rdisk4").may_be_listed());
        assert!(Media::from_url("https://a/b.mp3").may_be_listed());
        assert!(!Media::from_url("screen://").may_be_listed());
        assert!(!Media::from_url("v4l2:///dev/video0").may_be_listed());
        assert!(!Media::from_url("javascript://x").may_be_listed());
    }
}
