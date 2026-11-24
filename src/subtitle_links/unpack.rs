//! Downloaded subtitles that come packed (S6.5 edge case): a gzip file is unpacked, and a zip
//! archive gives its first subtitle file. Anything else is taken as it is.
//!
//! What comes out is held to the download limit, so a small archive can't unpack into gigabytes.
//! Only zip's stored and deflated entries are read: the methods subtitle archives use.

use std::io::Read as _;
use std::path::Path;

use flate2::read::{DeflateDecoder, GzDecoder};

use super::download::{Failure, MAX_BYTES};
use crate::engine::sidecar;

/// How a gzip file starts.
const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];
/// The signatures of a zip archive's local file header, central directory entry and its end.
const ZIP_LOCAL: u32 = 0x0403_4b50;
const ZIP_ENTRY: u32 = 0x0201_4b50;
const ZIP_END: u32 = 0x0605_4b50;
/// The fixed parts of those three records.
const LOCAL_SIZE: usize = 30;
const ENTRY_SIZE: usize = 46;
const END_SIZE: usize = 22;
/// The longest comment a zip archive's end record may carry.
const MAX_COMMENT: usize = u16::MAX as usize;
/// Zip's compression methods: none, and deflate.
const STORED: u16 = 0;
const DEFLATED: u16 = 8;

/// The file `bytes` hold, downloaded as `name`: unpacked when they are gzip or zip, with the
/// name of what was inside.
pub(super) fn unpack(name: &str, bytes: Vec<u8>) -> Result<(String, Vec<u8>), Failure> {
    if bytes.starts_with(&GZIP_MAGIC) {
        let inner = name.strip_suffix(".gz").unwrap_or(name).to_owned();
        return Ok((inner, limited(GzDecoder::new(bytes.as_slice()))?));
    }
    if read_u32(&bytes, 0) == Some(ZIP_LOCAL) {
        return first_subtitle(&bytes).ok_or(Failure::NotSubtitles)?;
    }
    Ok((name.to_owned(), bytes))
}

/// Everything `reader` gives, refused past [`MAX_BYTES`].
fn limited(reader: impl std::io::Read) -> Result<Vec<u8>, Failure> {
    let mut out = Vec::new();
    reader
        .take(MAX_BYTES + 1)
        .read_to_end(&mut out)
        .map_err(|error| Failure::Network(tr!("couldn't unpack it ({error})", error)))?;
    if out.len() as u64 > MAX_BYTES {
        return Err(Failure::TooBig);
    }
    Ok(out)
}

/// The first entry of the zip archive `bytes` named like a subtitle file, and its name.
/// `None` when there is none, or the archive can't be read.
fn first_subtitle(bytes: &[u8]) -> Option<Result<(String, Vec<u8>), Failure>> {
    let end = find_end(bytes)?;
    let count = usize::from(read_u16(bytes, end + 10)?);
    let mut at = read_u32(bytes, end + 16)? as usize;
    for _ in 0..count {
        let entry = Entry::read(bytes, at)?;
        at = entry.next;
        if let Some(name) = entry.subtitle_name() {
            return Some(entry.contents(bytes).map(|data| (name, data)));
        }
    }
    None
}

/// Where the archive's end record starts: searched for from the end, past its comment.
fn find_end(bytes: &[u8]) -> Option<usize> {
    let last = bytes.len().checked_sub(END_SIZE)?;
    let first = last.saturating_sub(MAX_COMMENT);
    (first..=last).rev().find(|&at| read_u32(bytes, at) == Some(ZIP_END))
}

/// One entry of a zip archive's central directory.
struct Entry {
    name: String,
    method: u16,
    packed_size: usize,
    local_header: usize,
    /// Where the next entry starts.
    next: usize,
}

impl Entry {
    fn read(bytes: &[u8], at: usize) -> Option<Self> {
        if read_u32(bytes, at)? != ZIP_ENTRY {
            return None;
        }
        let name_length = usize::from(read_u16(bytes, at + 28)?);
        let extra_length = usize::from(read_u16(bytes, at + 30)?);
        let comment_length = usize::from(read_u16(bytes, at + 32)?);
        let name_start = at + ENTRY_SIZE;
        let name = bytes.get(name_start..name_start + name_length)?;
        Some(Self {
            name: String::from_utf8_lossy(name).into_owned(),
            method: read_u16(bytes, at + 10)?,
            packed_size: read_u32(bytes, at + 20)? as usize,
            local_header: read_u32(bytes, at + 42)? as usize,
            next: name_start + name_length + extra_length + comment_length,
        })
    }

    /// The entry's file name when it names a subtitle file, leaving out folders and macOS's
    /// `__MACOSX` copies.
    fn subtitle_name(&self) -> Option<String> {
        if self.name.starts_with("__MACOSX/") || self.name.ends_with('/') {
            return None;
        }
        let name = self.name.rsplit(['/', '\\']).next()?;
        let wanted = !name.starts_with('.') && sidecar::has_subtitle_extension(Path::new(name));
        wanted.then(|| name.to_owned())
    }

    /// The entry's unpacked contents.
    fn contents(&self, bytes: &[u8]) -> Result<Vec<u8>, Failure> {
        let at = self.local_header;
        let start = (read_u32(bytes, at) == Some(ZIP_LOCAL))
            .then(|| {
                let name_length = usize::from(read_u16(bytes, at + 26)?);
                let extra_length = usize::from(read_u16(bytes, at + 28)?);
                Some(at + LOCAL_SIZE + name_length + extra_length)
            })
            .flatten()
            .ok_or(Failure::NotSubtitles)?;
        let packed = bytes.get(start..start + self.packed_size).ok_or(Failure::NotSubtitles)?;
        match self.method {
            STORED => limited(packed),
            DEFLATED => limited(DeflateDecoder::new(packed)),
            _ => Err(Failure::NotSubtitles),
        }
    }
}

fn read_u16(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn read_u32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use flate2::Compression;
    use flate2::write::{DeflateEncoder, GzEncoder};

    use super::*;

    const SRT: &[u8] = b"1\n00:00:01,000 --> 00:00:02,000\nHi\n";

    /// A zip archive of `entries` (name, method, contents as stored).
    fn zip(entries: &[(&str, u16, Vec<u8>)]) -> Vec<u8> {
        let (mut out, mut directory) = (Vec::new(), Vec::new());
        for (name, method, data) in entries {
            let offset = out.len() as u32;
            out.extend(header(ZIP_LOCAL, *method, data, name, None));
            out.extend(data);
            directory.extend(header(ZIP_ENTRY, *method, data, name, Some(offset)));
        }
        let directory_at = out.len() as u32;
        out.extend(&directory);
        out.extend(ZIP_END.to_le_bytes());
        out.extend([0u8; 4]);
        out.extend((entries.len() as u16).to_le_bytes());
        out.extend((entries.len() as u16).to_le_bytes());
        out.extend((directory.len() as u32).to_le_bytes());
        out.extend(directory_at.to_le_bytes());
        out.extend([0u8; 2]);
        out
    }

    /// A local header, or a central directory entry when it has the local header's `offset`.
    fn header(
        signature: u32,
        method: u16,
        data: &[u8],
        name: &str,
        offset: Option<u32>,
    ) -> Vec<u8> {
        let central = offset.is_some();
        let mut out = signature.to_le_bytes().to_vec();
        out.extend(vec![0u8; if central { 6 } else { 4 }]);
        out.extend(method.to_le_bytes());
        out.extend([0u8; 8]);
        out.extend((data.len() as u32).to_le_bytes());
        out.extend((data.len() as u32).to_le_bytes());
        out.extend((name.len() as u16).to_le_bytes());
        out.extend(vec![0u8; if central { 12 } else { 2 }]);
        out.extend(offset.map_or_else(Vec::new, |offset| offset.to_le_bytes().to_vec()));
        out.extend(name.as_bytes());
        out
    }

    #[test]
    fn plain_files_pass_through() {
        assert_eq!(unpack("a.srt", SRT.to_vec()), Ok(("a.srt".to_owned(), SRT.to_vec())));
    }

    #[test]
    fn gzip_is_unpacked() {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(SRT).unwrap();
        let packed = encoder.finish().unwrap();
        assert_eq!(unpack("a.srt.gz", packed), Ok(("a.srt".to_owned(), SRT.to_vec())));
    }

    #[test]
    fn a_zip_gives_its_first_subtitle_file() {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(SRT).unwrap();
        let archive = zip(&[
            ("readme.txt", STORED, b"hello".to_vec()),
            ("__MACOSX/._Movie.srt", STORED, b"junk".to_vec()),
            ("Subs/Movie.en.srt", DEFLATED, encoder.finish().unwrap()),
        ]);
        assert_eq!(unpack("x.zip", archive), Ok(("Movie.en.srt".to_owned(), SRT.to_vec())));
    }

    #[test]
    fn a_zip_without_subtitles_is_refused() {
        let archive = zip(&[("readme.txt", STORED, b"hello".to_vec())]);
        assert_eq!(unpack("x.zip", archive), Err(Failure::NotSubtitles));
        assert_eq!(unpack("x.zip", ZIP_LOCAL.to_le_bytes().to_vec()), Err(Failure::NotSubtitles));
    }
}
