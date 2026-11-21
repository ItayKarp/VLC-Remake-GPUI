//! Media identity (S1.7.2): a fast content key, so a moved or renamed file keeps its history and
//! copies, hard links and the same file through two mount points are one media.
//!
//! The key is xxh3-128 of the size, the first [`SAMPLE_BYTES`] and the last [`SAMPLE_BYTES`]:
//! two reads, whatever the file's size. Media files that agree on all of those are, in practice,
//! the same file.

use std::fs::{File, Metadata};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use std::time::UNIX_EPOCH;

use xxhash_rust::xxh3::Xxh3;

use crate::db::media::FileStamp;

/// How much of each end of a file the key reads.
pub const SAMPLE_BYTES: u64 = 64 * 1024;
/// Starts a file's identity, naming the hash so a later scheme can't be mistaken for it.
const FILE_PREFIX: &str = "xxh3:";
/// Starts a URL's identity.
const URL_PREFIX: &str = "url:";

/// The identity of the file at `path`, which is `size` bytes long.
pub fn file_key(path: &Path, size: u64) -> io::Result<String> {
    content_key(&mut File::open(path)?, size)
}

/// The identity of a stream: the URL itself.
pub fn url_key(url: &str) -> String {
    format!("{URL_PREFIX}{url}")
}

/// The content key of `reader`, which holds `size` bytes.
pub fn content_key(reader: &mut (impl Read + Seek), size: u64) -> io::Result<String> {
    let mut hasher = Xxh3::new();
    hasher.update(&size.to_le_bytes());
    let head = size.min(SAMPLE_BYTES);
    hash_range(reader, 0, head, &mut hasher)?;
    // The tail never overlaps the head, so a small file is read once.
    let tail = (size - head).min(SAMPLE_BYTES);
    hash_range(reader, size - tail, tail, &mut hasher)?;
    Ok(format!("{FILE_PREFIX}{:032x}", hasher.digest128()))
}

/// The size and modification time in `metadata`.
pub fn stamp(metadata: &Metadata) -> FileStamp {
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|since| i64::try_from(since.as_millis()).ok());
    FileStamp { size: metadata.len(), modified_ms }
}

fn hash_range(
    reader: &mut (impl Read + Seek),
    start: u64,
    length: u64,
    hasher: &mut Xxh3,
) -> io::Result<()> {
    if length == 0 {
        return Ok(());
    }
    reader.seek(SeekFrom::Start(start))?;
    let mut buffer = vec![0; length as usize];
    reader.read_exact(&mut buffer)?;
    hasher.update(&buffer);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn key(bytes: &[u8]) -> String {
        content_key(&mut Cursor::new(bytes), bytes.len() as u64).expect("in-memory read")
    }

    fn big(fill: u8) -> Vec<u8> {
        vec![fill; 3 * SAMPLE_BYTES as usize]
    }

    #[test]
    fn the_same_bytes_give_the_same_key() {
        assert_eq!(key(b"movie"), key(b"movie"));
        assert!(key(b"movie").starts_with(FILE_PREFIX));
        assert_eq!(key(&big(1)), key(&big(1)));
    }

    #[test]
    fn the_head_tail_and_size_all_count() {
        let base = big(0);
        let mut head = base.clone();
        head[10] = 1;
        let mut tail = base.clone();
        *tail.last_mut().expect("not empty") = 1;
        let mut longer = base.clone();
        longer.push(0);
        for changed in [head, tail, longer] {
            assert_ne!(key(&base), key(&changed));
        }
    }

    #[test]
    fn the_middle_of_a_large_file_is_not_read() {
        let base = big(0);
        let mut middle = base.clone();
        middle[base.len() / 2] = 1;
        assert_eq!(key(&base), key(&middle));
    }

    #[test]
    fn empty_and_small_files_have_keys() {
        assert_ne!(key(b""), key(b"a"));
        assert_ne!(key(b"ab"), key(b"ba"));
    }

    #[test]
    fn urls_are_their_own_identity() {
        assert_eq!(url_key("https://x/live"), "url:https://x/live");
    }
}
