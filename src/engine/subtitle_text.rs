//! The encoding of text subtitle files (S6.1.6): a byte order mark first, then UTF-8, then a
//! statistical guess (Firefox's `chardetng`), or the encoding the user chose for the file.
//!
//! libVLC 3 reads one encoding for all of a media's subtitles (`subsdec-encoding`), and without
//! one tries UTF-8 and falls back to the interface language's code page. So a file it would
//! misread is converted here: decoded in its encoding and written as UTF-8 with a byte order
//! mark, which libVLC's subtitle reader honors over any encoding option, to a folder of its own
//! in the cache under the same name (libVLC reads a language from the name). A file libVLC reads
//! right, and every file while Settings → Subtitles names an encoding, loads as it is.

use std::fs;
use std::path::{Path, PathBuf};

use encoding_rs::{Encoding, UTF_8};
use xxhash_rust::xxh3::xxh3_64;

/// The largest file read to guess its encoding; bigger ones load as they are (the size limit of
/// text subtitles, S6.1.2).
const MAX_BYTES: u64 = 32 * 1024 * 1024;
/// UTF-8's byte order mark.
const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";

/// How a file's encoding is decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodingChoice {
    /// From the file: its byte order mark, UTF-8, or a guess.
    Guess,
    /// Settings → Subtitles names one for every file: libVLC reads it.
    Setting,
    /// The user chose this one for the file.
    Use(&'static Encoding),
}

/// A subtitle file as it loads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prepared {
    /// The file to give libVLC: the file itself, or its UTF-8 copy.
    pub path: PathBuf,
    /// The encoding it is read in, when known.
    pub encoding: Option<&'static Encoding>,
}

impl Prepared {
    fn as_is(path: &Path, encoding: Option<&'static Encoding>) -> Self {
        Self { path: path.to_path_buf(), encoding }
    }
}

/// Decides how the subtitle file at `path` is read, converting it into `converted` (a cache
/// folder) when libVLC would misread it. Reads the file, so it belongs off the UI thread. A
/// picture format, an unreadable or binary file, or no cache folder loads the file as it is.
pub fn prepare(path: &Path, choice: EncodingChoice, converted: Option<&Path>) -> Prepared {
    if choice == EncodingChoice::Setting || !super::sidecar::is_text(path) {
        return Prepared::as_is(path, None);
    }
    let Some(bytes) = read(path) else { return Prepared::as_is(path, None) };
    let encoding = match choice {
        EncodingChoice::Use(encoding) => encoding,
        _ => match marked_or_utf8(&bytes) {
            Some(encoding) => return Prepared::as_is(path, Some(encoding)),
            None => guess(&bytes),
        },
    };
    let Some(folder) = converted else { return Prepared::as_is(path, Some(encoding)) };
    match convert(path, &bytes, encoding, folder) {
        Ok(copy) => Prepared { path: copy, encoding: Some(encoding) },
        Err(error) => {
            let shown = crate::logging::path(path);
            tracing::warn!("subtitles: couldn't convert {shown} from {}: {error}", encoding.name());
            Prepared::as_is(path, Some(encoding))
        }
    }
}

/// The file's bytes, unless it is too big or holds binary data (NUL bytes outside UTF-16).
fn read(path: &Path) -> Option<Vec<u8>> {
    let size = fs::metadata(path).ok()?.len();
    if size == 0 || size > MAX_BYTES {
        return None;
    }
    let bytes = fs::read(path).ok()?;
    let utf16 = matches!(Encoding::for_bom(&bytes), Some((encoding, _)) if encoding != UTF_8);
    (utf16 || !bytes.contains(&0)).then_some(bytes)
}

/// The encoding a byte order mark names, or UTF-8 when the bytes are valid UTF-8 (plain ASCII
/// included): both of which libVLC reads by itself.
fn marked_or_utf8(bytes: &[u8]) -> Option<&'static Encoding> {
    match Encoding::for_bom(bytes) {
        Some((encoding, _)) => Some(encoding),
        None => std::str::from_utf8(bytes).is_ok().then_some(UTF_8),
    }
}

/// The encoding unmarked, non-UTF-8 text is most likely in.
fn guess(bytes: &[u8]) -> &'static Encoding {
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(bytes, true);
    detector.guess(None, false)
}

/// Writes `bytes`, read in `encoding`, as UTF-8 with a byte order mark to a folder of its own in
/// `folder`, named for the file and the encoding, keeping the file's name.
fn convert(
    path: &Path,
    bytes: &[u8],
    encoding: &'static Encoding,
    folder: &Path,
) -> std::io::Result<PathBuf> {
    let name = path.file_name().ok_or(std::io::ErrorKind::InvalidInput)?;
    let key = format!("{}\0{}", path.display(), encoding.name());
    let folder = folder.join(format!("{:016x}", xxh3_64(key.as_bytes())));
    fs::create_dir_all(&folder)?;
    let (text, _) = encoding.decode_with_bom_removal(bytes);
    let copy = folder.join(name);
    fs::write(&copy, [UTF8_BOM, text.as_bytes()].concat())?;
    Ok(copy)
}

#[cfg(test)]
mod tests;
