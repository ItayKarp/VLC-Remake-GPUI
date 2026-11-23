//! The OpenSubtitles "moviehash" of a video (S6.5.2): its size plus the sum of its first and
//! last 64 KB read as little-endian 64-bit words, wrapping, as 16 hex digits. It names the exact
//! file, so subtitles made for that release sort first.
//!
//! Only those 128 KB are read, on a thread of its own: a file on a slow network share that
//! doesn't answer in time searches by title alone.

use std::fs::File;
use std::io::{self, Read as _, Seek as _, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// The bytes read from each end of the file.
const CHUNK: usize = 64 * 1024;
/// How long reading them may take.
const TIMEOUT: Duration = Duration::from_secs(5);

/// The moviehash of the file at `path`, or `None` when it is smaller than 64 KB, can't be read,
/// or takes longer than [`TIMEOUT`].
pub(super) fn of(path: &Path) -> Option<String> {
    let (sender, receiver) = mpsc::channel();
    let path: PathBuf = path.to_path_buf();
    let spawned = thread::Builder::new().name("moviehash".to_owned()).spawn(move || {
        let _ = sender.send(compute(&path));
    });
    if let Err(error) = spawned {
        tracing::warn!("opensubtitles: no thread to hash the file: {error}");
        return None;
    }
    let Ok(computed) = receiver.recv_timeout(TIMEOUT) else {
        tracing::info!("opensubtitles: the file took too long to hash; searching by title");
        return None;
    };
    computed
        .inspect_err(|error| tracing::debug!("opensubtitles: no file hash: {error}"))
        .ok()
        .map(|hash| format!("{hash:016x}"))
}

fn compute(path: &Path) -> io::Result<u64> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    if size < CHUNK as u64 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "too small to hash"));
    }
    let mut buffer = vec![0; CHUNK];
    file.read_exact(&mut buffer)?;
    let head = sum(&buffer);
    file.seek(SeekFrom::Start(size - CHUNK as u64))?;
    file.read_exact(&mut buffer)?;
    Ok(size.wrapping_add(head).wrapping_add(sum(&buffer)))
}

/// The wrapping sum of `bytes` read as little-endian 64-bit words.
fn sum(bytes: &[u8]) -> u64 {
    let (words, _) = bytes.as_chunks::<8>();
    words.iter().fold(0, |total, word| total.wrapping_add(u64::from_le_bytes(*word)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opening::scratch::scratch_dir;

    #[test]
    fn the_hash_is_the_size_plus_both_ends() {
        let path = scratch_dir().join("video.mkv");
        let mut bytes = vec![0u8; 3 * CHUNK];
        bytes[0] = 1;
        bytes[3 * CHUNK - 8] = 2;
        bytes[CHUNK + 5] = 9;
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(of(&path), Some(format!("{:016x}", 3 * CHUNK as u64 + 3)));
    }

    #[test]
    fn a_small_or_missing_file_has_none() {
        let folder = scratch_dir();
        std::fs::write(folder.join("small.mkv"), b"tiny").unwrap();
        assert_eq!(of(&folder.join("small.mkv")), None);
        assert_eq!(of(&folder.join("gone.mkv")), None);
    }

    #[test]
    fn the_sum_wraps() {
        assert_eq!(sum(&[0xff; 16]), u64::MAX.wrapping_mul(2));
    }
}
