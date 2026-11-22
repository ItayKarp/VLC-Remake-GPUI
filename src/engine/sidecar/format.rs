//! Which files hold subtitles (S6.1.2), and whether one is what its name says: a size limit and a
//! look at its first bytes, so a 50 MB "SRT" that is really a video, or any binary file, never
//! reaches libVLC's parsers.

use std::fs::File;
use std::io::Read;
use std::path::Path;

/// The most a text subtitle file may hold. A feature film's SRT is about 100 KB, and an ASS file
/// with karaoke effects or embedded fonts a few MB.
const MAX_TEXT_BYTES: u64 = 32 * 1024 * 1024;
/// The most an image subtitle file (PGS, VobSub) may hold: a Blu-ray's PGS track can pass
/// 100 MB.
const MAX_IMAGE_BYTES: u64 = 512 * 1024 * 1024;
/// How much of a file is read to check it.
const HEAD_BYTES: u64 = 8 * 1024;
/// The byte order marks of UTF-16, little-endian then big-endian.
const UTF16_LE: [u8; 2] = [0xFF, 0xFE];
const UTF16_BE: [u8; 2] = [0xFE, 0xFF];

/// A subtitle file format, as its extension names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Format {
    /// SubRip (`.srt`).
    SubRip,
    /// Advanced SubStation Alpha and SubStation Alpha (`.ass`, `.ssa`).
    SubStation,
    /// WebVTT (`.vtt`).
    WebVtt,
    /// MicroDVD or SubViewer text (`.sub`). A `.sub` beside an `.idx` is VobSub's pictures.
    TextSub,
    /// VobSub's index (`.idx`), which opens the pictures in the `.sub` beside it.
    VobSub,
    /// Blu-ray PGS pictures (`.sup`).
    Pgs,
    /// Timed Text (`.ttml`, `.dfxp`).
    Ttml,
    /// SAMI (`.smi`, `.sami`).
    Sami,
    /// The other text formats libVLC reads (MPL2, AQTitle, PJS, JACOSub, RealText,
    /// PowerDivX).
    OtherText,
}

impl Format {
    /// The format a file with `extension` holds; `None` when it isn't a subtitle file.
    pub(super) fn from_extension(extension: &str) -> Option<Self> {
        let format = match extension.to_ascii_lowercase().as_str() {
            "srt" => Self::SubRip,
            "ass" | "ssa" => Self::SubStation,
            "vtt" => Self::WebVtt,
            "sub" => Self::TextSub,
            "idx" => Self::VobSub,
            "sup" => Self::Pgs,
            "ttml" | "dfxp" => Self::Ttml,
            "smi" | "sami" => Self::Sami,
            "mpl" | "aqt" | "pjs" | "jss" | "rt" | "psb" => Self::OtherText,
            _ => return None,
        };
        Some(format)
    }

    /// Whether the format is text, read in some encoding.
    pub(super) fn is_text(self) -> bool {
        !matches!(self, Self::Pgs | Self::VobSub)
    }

    /// Whether the file at `path` is no bigger than the format allows and starts the way it
    /// should.
    pub(super) fn checks_out(self, path: &Path) -> bool {
        let Ok(file) = File::open(path) else { return false };
        let size = file.metadata().map_or(0, |metadata| metadata.len());
        if size == 0 || size > self.max_bytes() {
            return false;
        }
        let mut head = Vec::new();
        if file.take(HEAD_BYTES).read_to_end(&mut head).is_err() {
            return false;
        }
        self.accepts(&head)
    }

    fn max_bytes(self) -> u64 {
        match self {
            Self::Pgs => MAX_IMAGE_BYTES,
            _ => MAX_TEXT_BYTES,
        }
    }

    /// Whether a file starting with `head` can be this format.
    fn accepts(self, head: &[u8]) -> bool {
        if self == Self::Pgs {
            return head.starts_with(b"PG");
        }
        let Some(text) = text(head) else { return false };
        let text = text.trim_start_matches('\u{feff}').trim_start().to_lowercase();
        match self {
            Self::SubRip => text.contains("-->"),
            Self::SubStation => text.contains("[script info]") || text.contains("[v4"),
            Self::WebVtt => text.starts_with("webvtt"),
            Self::VobSub => text.contains("vobsub index"),
            Self::Ttml => text.contains("<tt"),
            Self::Sami => text.contains("<sami"),
            Self::TextSub | Self::OtherText | Self::Pgs => true,
        }
    }
}

/// `head` as text: UTF-16 with a byte order mark decoded, anything else read as UTF-8 with
/// unreadable bytes replaced (an 8-bit encoding still shows its ASCII). `None` for binary data,
/// which has NUL bytes no text file has.
fn text(head: &[u8]) -> Option<String> {
    let utf16 = |to_unit: fn([u8; 2]) -> u16| {
        let units: Vec<u16> =
            head[2..].as_chunks::<2>().0.iter().map(|pair| to_unit(*pair)).collect();
        String::from_utf16_lossy(&units)
    };
    if head.starts_with(&UTF16_LE) {
        return Some(utf16(u16::from_le_bytes));
    }
    if head.starts_with(&UTF16_BE) {
        return Some(utf16(u16::from_be_bytes));
    }
    (!head.contains(&0)).then(|| String::from_utf8_lossy(head).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_name_formats_in_any_case() {
        assert_eq!(Format::from_extension("SRT"), Some(Format::SubRip));
        assert_eq!(Format::from_extension("ssa"), Some(Format::SubStation));
        assert_eq!(Format::from_extension("dfxp"), Some(Format::Ttml));
        assert_eq!(Format::from_extension("mkv"), None);
        assert_eq!(Format::from_extension("txt"), None);
    }

    #[test]
    fn each_format_is_known_by_its_start() {
        let srt = b"\xEF\xBB\xBF1\r\n00:00:01,000 --> 00:00:02,000\r\nHello\r\n";
        assert!(Format::SubRip.accepts(srt));
        assert!(!Format::SubRip.accepts(b"just some notes"));
        assert!(Format::SubStation.accepts(b"[Script Info]\nTitle: x"));
        assert!(Format::WebVtt.accepts(b"\nWEBVTT\n\n00:01.000 --> 00:02.000\nHi"));
        assert!(Format::VobSub.accepts(b"# VobSub index file, v7 (do not modify this line!)"));
        assert!(Format::Pgs.accepts(b"PG\x00\x00\x01"));
        assert!(!Format::Pgs.accepts(b"\x1A\x45\xDF\xA3"));
    }

    #[test]
    fn binary_data_is_never_text() {
        assert!(!Format::SubRip.accepts(b"\x1A\x45\xDF\xA3\x00\x00 --> "));
        assert!(!Format::TextSub.accepts(b"\x00\x00\x01\xBA"));
    }

    #[test]
    fn utf16_text_is_read_by_its_byte_order_mark() {
        let le: Vec<u8> = UTF16_LE
            .into_iter()
            .chain("1\n00:00:01,000 --> 2".encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        assert!(Format::SubRip.accepts(&le));
        let be: Vec<u8> = UTF16_BE
            .into_iter()
            .chain("WEBVTT".encode_utf16().flat_map(u16::to_be_bytes))
            .collect();
        assert!(Format::WebVtt.accepts(&be));
    }
}
