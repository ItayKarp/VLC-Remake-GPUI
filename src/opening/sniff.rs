//! Identifies a file by its first bytes (S1.3.6), for files whose extension says nothing: no
//! extension, an unknown one, or a wrong one. Only signatures that are hard to hit by accident
//! are matched; anything else is left to the caller.

use super::kind::{Kind, PlaylistFormat};

/// An MPEG transport stream packet's size and sync byte.
const TS_PACKET: usize = 188;
const TS_SYNC: u8 = 0x47;
/// How many packets in a row must start with the sync byte to call it a transport stream.
const TS_PACKETS_CHECKED: usize = 3;
/// Blu-ray `.m2ts` packets carry a 4-byte timestamp before each 188-byte packet.
const M2TS_PREFIX: usize = 4;

/// Byte signatures at the start of a media file.
const MEDIA_MAGIC: &[&[u8]] = &[
    b"\x1A\x45\xDF\xA3",                 // Matroska, WebM
    b"OggS",                             // Ogg
    b"fLaC",                             // FLAC
    b"ID3",                              // MP3 with an ID3v2 tag
    b"FLV",                              // Flash video
    b"\x00\x00\x01\xBA",                 // MPEG program stream (VOB)
    b"\x00\x00\x01\xB3",                 // MPEG-1/2 video
    b"\x30\x26\xB2\x75\x8E\x66\xCF\x11", // ASF (WMV, WMA)
    b".RMF",                             // RealMedia
    b"MAC ",                             // Monkey's Audio
    b"wvpk",                             // WavPack
    b"MPCK",                             // Musepack
    b"#!AMR",                            // AMR speech
    b"caff",                             // Core Audio
    b".snd",                             // Sun audio
];

/// ISO base media (MP4, MOV, 3GP, M4A) box types that open a file, at offset 4.
const ISO_BOXES: &[&[u8]] = &[b"ftyp", b"moov", b"mdat", b"free", b"wide", b"skip"];

/// What `head` (a file's first bytes) is, if it is recognizable.
pub fn kind(head: &[u8]) -> Option<Kind> {
    // A UTF-16 byte order mark also reads as an MPEG audio frame header.
    let utf16 = head.starts_with(&[0xFF, 0xFE]) || head.starts_with(&[0xFE, 0xFF]);
    if !utf16 && is_media(head) {
        return Some(Kind::Media);
    }
    text_kind(&decode_head(head))
}

fn is_media(head: &[u8]) -> bool {
    MEDIA_MAGIC.iter().any(|magic| head.starts_with(magic))
        || head.get(4..8).is_some_and(|tag| ISO_BOXES.contains(&tag))
        || is_riff_or_iff(head)
        || is_mpeg_audio(head)
        || is_transport_stream(head, 0)
        || is_transport_stream(head, M2TS_PREFIX)
}

/// RIFF (AVI, WAV, VCD) and IFF (AIFF) containers with a media form type.
fn is_riff_or_iff(head: &[u8]) -> bool {
    let (Some(container), Some(form)) = (head.get(..4), head.get(8..12)) else {
        return false;
    };
    match container {
        b"RIFF" => matches!(form, b"AVI " | b"WAVE" | b"CDXA"),
        b"FORM" => matches!(form, b"AIFF" | b"AIFC"),
        _ => false,
    }
}

/// An MPEG audio frame (MP3 without a tag) or an AAC ADTS frame: 11 or 12 sync bits.
fn is_mpeg_audio(head: &[u8]) -> bool {
    let [0xFF, second, ..] = *head else { return false };
    let mpeg = second & 0xE0 == 0xE0 && second & 0x06 != 0;
    let adts = second & 0xF6 == 0xF0;
    mpeg || adts
}

/// Sync bytes every 188 bytes from `offset`, for a few packets in a row.
fn is_transport_stream(head: &[u8], offset: usize) -> bool {
    let stride = TS_PACKET + offset;
    (0..TS_PACKETS_CHECKED).all(|i| head.get(offset + i * stride) == Some(&TS_SYNC))
}

/// The head as text: UTF-8 (without a BOM) or UTF-16 with a BOM; invalid bytes become U+FFFD.
fn decode_head(head: &[u8]) -> String {
    match head {
        [0xFF, 0xFE, rest @ ..] => utf16(rest, u16::from_le_bytes),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, u16::from_be_bytes),
        _ => String::from_utf8_lossy(head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head)).into(),
    }
}

fn utf16(bytes: &[u8], unit: fn([u8; 2]) -> u16) -> String {
    let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|pair| unit(*pair)).collect();
    String::from_utf16_lossy(&units)
}

/// Playlists and subtitles are text; recognize them by their first lines.
fn text_kind(text: &str) -> Option<Kind> {
    let trimmed = text.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    let playlist = |format| Some(Kind::Playlist(format));
    if lower.starts_with("#extm3u") {
        return playlist(PlaylistFormat::M3u);
    }
    if lower.starts_with("[playlist]") {
        return playlist(PlaylistFormat::Pls);
    }
    if lower.starts_with("<asx") || lower.starts_with("<?xml") && lower.contains("<asx") {
        return playlist(PlaylistFormat::Asx);
    }
    if lower.starts_with("<?xml") && lower.contains("<playlist") && lower.contains("xspf") {
        return playlist(PlaylistFormat::Xspf);
    }
    if lower.starts_with("webvtt") || lower.starts_with("[script info]") || is_srt(trimmed) {
        return Some(Kind::Subtitle);
    }
    is_cue(&lower).then_some(Kind::Playlist(PlaylistFormat::Cue))
}

/// SubRip: a cue number, then a `00:00:01,000 --> 00:00:04,000` line.
fn is_srt(text: &str) -> bool {
    let mut lines = text.lines().map(str::trim);
    let number =
        lines.next().is_some_and(|l| !l.is_empty() && l.bytes().all(|b| b.is_ascii_digit()));
    number && lines.next().is_some_and(|l| l.contains("-->"))
}

/// A cue sheet names its audio in a `FILE` line and lists `TRACK`s after it.
fn is_cue(lower: &str) -> bool {
    let mut lines = lower.lines().map(str::trim_start);
    lines.any(|line| line.starts_with("file ")) && lines.any(|line| line.starts_with("track "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn media(head: &[u8]) -> bool {
        kind(head) == Some(Kind::Media)
    }

    #[test]
    fn recognizes_containers() {
        assert!(media(b"\x1A\x45\xDF\xA3\x01\x00"));
        assert!(media(b"\x00\x00\x00\x20ftypisom"));
        assert!(media(b"RIFF\x00\x00\x00\x00AVI LIST"));
        assert!(media(b"RIFF\x00\x00\x00\x00WAVEfmt "));
        assert!(media(b"FORM\x00\x00\x00\x00AIFF"));
        assert!(media(b"ID3\x04\x00"));
        assert!(media(&[0xFF, 0xFB, 0x90, 0x00]));
        assert!(media(&[0xFF, 0xF1, 0x50, 0x80]));
        assert!(!media(b"RIFF\x00\x00\x00\x00WEBPVP8 "));
    }

    #[test]
    fn recognizes_transport_streams() {
        let mut ts = vec![0; TS_PACKET * 3];
        for i in 0..3 {
            ts[i * TS_PACKET] = TS_SYNC;
        }
        assert!(media(&ts));
        let mut m2ts = vec![0; (TS_PACKET + M2TS_PREFIX) * 3];
        for i in 0..3 {
            m2ts[M2TS_PREFIX + i * (TS_PACKET + M2TS_PREFIX)] = TS_SYNC;
        }
        assert!(media(&m2ts));
        assert!(!media(&[TS_SYNC; 10]));
    }

    #[test]
    fn recognizes_playlists() {
        let playlist = |text: &[u8]| match kind(text) {
            Some(Kind::Playlist(format)) => Some(format),
            _ => None,
        };
        assert_eq!(playlist(b"\xEF\xBB\xBF#EXTM3U\n#EXTINF:1,a\na.mp3"), Some(PlaylistFormat::M3u));
        assert_eq!(playlist(b"[Playlist]\nFile1=a.mp3"), Some(PlaylistFormat::Pls));
        assert_eq!(
            playlist(b"<?xml version=\"1.0\"?>\n<playlist xmlns=\"http://xspf.org/ns/0/\">"),
            Some(PlaylistFormat::Xspf)
        );
        assert_eq!(playlist(b"<ASX version=\"3.0\">"), Some(PlaylistFormat::Asx));
        assert_eq!(
            playlist(b"REM GENRE Rock\nFILE \"a.flac\" WAVE\n  TRACK 01 AUDIO"),
            Some(PlaylistFormat::Cue)
        );
    }

    #[test]
    fn recognizes_subtitles_including_utf16() {
        assert_eq!(kind(b"1\r\n00:00:01,000 --> 00:00:02,000\r\nHi"), Some(Kind::Subtitle));
        assert_eq!(kind(b"WEBVTT\n\n00:01.000 --> 00:02.000"), Some(Kind::Subtitle));
        let utf16: Vec<u8> = [0xFF, 0xFE]
            .into_iter()
            .chain("[Script Info]".encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        assert_eq!(kind(&utf16), Some(Kind::Subtitle));
    }

    #[test]
    fn ignores_everything_else() {
        assert_eq!(kind(b"%PDF-1.7"), None);
        assert_eq!(kind(b"MZ\x90\x00"), None);
        assert_eq!(kind(b"just some notes\n2 lines"), None);
        assert_eq!(kind(b""), None);
    }
}
