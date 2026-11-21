//! Turning what libVLC reports about a file into the library's values: numbers and years from
//! tag text, codec names from FourCCs, the media kind from its tracks, and a cover's file from
//! its URL. Pure functions, so they are tested without libVLC.

use std::path::{Path, PathBuf};

use crate::db::metadata::{MediaKind, Resolution};

/// What one track of a file is, as far as the library cares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrackSummary {
    /// Whether it is video, audio or something else (subtitles).
    pub kind: TrackType,
    /// libVLC's FourCC for its codec.
    pub codec: u32,
    /// A video track's displayed size, when known.
    pub size: Option<Resolution>,
}

/// A track's type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackType {
    /// Pictures.
    Video,
    /// Sound.
    Audio,
    /// Subtitles, or a type libVLC doesn't know.
    Other,
}

/// Codecs whose "video" track is one still picture: a cover, not a video.
const STILL_IMAGE_CODECS: [&[u8; 4]; 4] = [b"jpeg", b"png ", b"bmp ", b"gif "];

/// Display names for the common codecs, by libVLC FourCC.
const CODEC_NAMES: [(&[u8; 4], &str); 31] = [
    (b"h264", "H.264"),
    (b"hevc", "HEVC"),
    (b"av01", "AV1"),
    (b"VP80", "VP8"),
    (b"VP90", "VP9"),
    (b"mp4v", "MPEG-4"),
    (b"mp1v", "MPEG-1"),
    (b"mp2v", "MPEG-2"),
    (b"mpgv", "MPEG-2"),
    (b"WMV2", "WMV"),
    (b"WMV3", "WMV"),
    (b"WVC1", "VC-1"),
    (b"theo", "Theora"),
    (b"MJPG", "MJPEG"),
    (b"mp4a", "AAC"),
    (b"mpga", "MP3"),
    (b"mp3 ", "MP3"),
    (b"a52 ", "AC-3"),
    (b"eac3", "E-AC-3"),
    (b"dts ", "DTS"),
    (b"trhd", "TrueHD"),
    (b"flac", "FLAC"),
    (b"opus", "Opus"),
    (b"vorb", "Vorbis"),
    (b"alac", "ALAC"),
    (b"wma2", "WMA"),
    (b"wmap", "WMA Pro"),
    (b"araw", "PCM"),
    (b"s16l", "PCM"),
    (b"s24l", "PCM"),
    (b"lpcm", "PCM"),
];

/// A track or disc number from tag text: `5`, `05` and `5/12` are 5. Zero and text that isn't
/// a number (`A1` on a vinyl rip) are none.
pub fn tag_number(text: &str) -> Option<u32> {
    let number = text.split('/').next()?.trim();
    number.parse().ok().filter(|&number| number > 0)
}

/// The year from a date tag: `2003`, `2003-05-01` and `2003-05-01T10:00:00` are 2003.
pub fn tag_year(text: &str) -> Option<i32> {
    let year = text.trim().get(..4)?;
    let year: i32 = year.parse().ok()?;
    (1..=9999).contains(&year).then_some(year)
}

/// The title tag, unless it is only the file's name: libVLC answers a file without a title tag
/// with its name, and the views show the name themselves.
pub fn tag_title(title: Option<String>, path: &Path) -> Option<String> {
    let title = title?;
    let is_name = |name: Option<&std::ffi::OsStr>| name.is_some_and(|name| name == title.as_str());
    (!is_name(path.file_name()) && !is_name(path.file_stem())).then_some(title)
}

/// Whether the media is video or audio: a video track that isn't a still picture makes it
/// video.
pub fn kind(tracks: &[TrackSummary]) -> MediaKind {
    if tracks.iter().any(is_moving_video) {
        MediaKind::Video
    } else if tracks.iter().any(|track| track.kind == TrackType::Audio) {
        MediaKind::Audio
    } else {
        MediaKind::Unknown
    }
}

/// The first video track's size.
pub fn resolution(tracks: &[TrackSummary]) -> Option<Resolution> {
    tracks.iter().filter(|track| is_moving_video(track)).find_map(|track| track.size)
}

/// The codecs, for display: the first video track's and the first audio track's,
/// `H.264 / AAC`.
pub fn codecs(tracks: &[TrackSummary]) -> Option<String> {
    let video = tracks.iter().find(|track| is_moving_video(track));
    let audio = tracks.iter().find(|track| track.kind == TrackType::Audio);
    let names: Vec<String> =
        [video, audio].into_iter().flatten().map(|t| codec_name(t.codec)).collect();
    (!names.is_empty()).then(|| names.join(" / "))
}

/// A codec's display name: its common name, or its FourCC in capitals.
pub fn codec_name(fourcc: u32) -> String {
    let bytes = fourcc.to_le_bytes();
    if let Some((_, name)) = CODEC_NAMES.iter().find(|(code, _)| **code == bytes) {
        return (*name).to_owned();
    }
    String::from_utf8_lossy(&bytes).trim().to_uppercase()
}

/// The file a cover URL names: libVLC reports a cover it extracted, or found next to the file,
/// as a `file://` URL. Covers it hasn't saved (`attachment://`) have no file.
pub fn artwork_path(url: &str) -> Option<PathBuf> {
    let url = url::Url::parse(url).ok()?;
    (url.scheme() == "file").then(|| url.to_file_path().ok()).flatten()
}

fn is_moving_video(track: &TrackSummary) -> bool {
    let codec = track.codec.to_le_bytes();
    track.kind == TrackType::Video && !STILL_IMAGE_CODECS.iter().any(|still| **still == codec)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fourcc(code: [u8; 4]) -> u32 {
        u32::from_le_bytes(code)
    }

    fn track(kind: TrackType, code: [u8; 4], size: Option<(u32, u32)>) -> TrackSummary {
        let size = size.map(|(width, height)| Resolution { width, height });
        TrackSummary { kind, codec: fourcc(code), size }
    }

    #[test]
    fn numbers_and_years_are_read_from_tag_text() {
        assert_eq!(tag_number("5"), Some(5));
        assert_eq!(tag_number("05/12"), Some(5));
        assert_eq!(tag_number("0"), None);
        assert_eq!(tag_number("A1"), None);
        assert_eq!(tag_year("2003"), Some(2003));
        assert_eq!(tag_year("2003-05-01T10:00:00"), Some(2003));
        assert_eq!(tag_year("03"), None);
        assert_eq!(tag_year("0000"), None);
    }

    #[test]
    fn a_title_that_is_only_the_file_name_is_no_title() {
        let path = Path::new("/m/Song.mp3");
        assert_eq!(tag_title(Some("Song.mp3".into()), path), None);
        assert_eq!(tag_title(Some("Song".into()), path), None);
        assert_eq!(tag_title(Some("A Song".into()), path), Some("A Song".into()));
        assert_eq!(tag_title(None, path), None);
    }

    #[test]
    fn tracks_give_the_kind_size_and_codecs() {
        let movie = [
            track(TrackType::Video, *b"h264", Some((1920, 1080))),
            track(TrackType::Audio, *b"mp4a", None),
            track(TrackType::Other, *b"subt", None),
        ];
        assert_eq!(kind(&movie), MediaKind::Video);
        assert_eq!(resolution(&movie), Some(Resolution { width: 1920, height: 1080 }));
        assert_eq!(codecs(&movie).as_deref(), Some("H.264 / AAC"));
        assert_eq!(kind(&[]), MediaKind::Unknown);
        assert_eq!(codecs(&[]), None);
    }

    #[test]
    fn a_cover_picture_track_doesnt_make_a_song_a_video() {
        let song = [
            track(TrackType::Video, *b"jpeg", Some((500, 500))),
            track(TrackType::Audio, *b"flac", None),
        ];
        assert_eq!(kind(&song), MediaKind::Audio);
        assert_eq!(resolution(&song), None);
        assert_eq!(codecs(&song).as_deref(), Some("FLAC"));
    }

    #[test]
    fn unknown_codecs_show_their_fourcc() {
        assert_eq!(codec_name(fourcc(*b"xyz ")), "XYZ");
        assert_eq!(codec_name(fourcc(*b"a52 ")), "AC-3");
    }

    #[test]
    fn only_saved_covers_have_a_file() {
        let saved = artwork_path("file:///tmp/art/cover.jpg");
        assert_eq!(saved, cfg!(unix).then(|| PathBuf::from("/tmp/art/cover.jpg")));
        assert_eq!(artwork_path("attachment://picture0_jpeg"), None);
        assert_eq!(artwork_path("not a url"), None);
    }
}
