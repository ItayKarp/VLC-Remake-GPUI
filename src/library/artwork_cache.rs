//! The library's cover cache (S4.3.1, S4.3.2): each cover image, downscaled to at most
//! [`MAX_SIDE`] pixels and saved as JPEG in the cache dir under a hash of the original, so an
//! album's tracks (and copies of one cover) share one file.
//!
//! Decoding a large picture (a 20 MB PNG scan) is slow, so this runs on the indexer's thread,
//! never the UI's. The cache can be deleted at any time; the views treat a missing file as no
//! cover.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use image::ImageReader;
use image::codecs::jpeg::JpegEncoder;
use xxhash_rust::xxh3::xxh3_128;

/// The longest side a cached cover keeps: twice the largest tile the designs show it in.
pub const MAX_SIDE: u32 = 600;
/// Covers bigger than this aren't read at all: no real cover is, and decoding one could take
/// most of the memory.
pub const MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
/// The quality cached covers are saved at.
const JPEG_QUALITY: u8 = 88;
/// The cached files' extension.
const EXTENSION: &str = "jpg";

/// The cover cache in one folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtworkCache {
    dir: PathBuf,
}

impl ArtworkCache {
    /// The cache in `dir`, created when the first cover is saved.
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Where the cover cached as `key` is.
    pub fn path(&self, key: &str) -> PathBuf {
        self.dir.join(key).with_extension(EXTENSION)
    }

    /// Caches the image at `source`, unless it already is: answers its key, or `None` when it
    /// can't be read, isn't a picture, or can't be saved.
    pub fn store(&self, source: &Path) -> Option<String> {
        self.try_store(source)
            .inspect_err(|error| {
                tracing::debug!("library: no cover from {}: {error}", crate::logging::path(source));
            })
            .ok()
    }

    fn try_store(&self, source: &Path) -> io::Result<String> {
        if fs::metadata(source)?.len() > MAX_SOURCE_BYTES {
            return Err(io::Error::other("too large for a cover"));
        }
        let bytes = fs::read(source)?;
        let key = format!("{:032x}", xxh3_128(&bytes));
        let path = self.path(&key);
        if !path.exists() {
            fs::create_dir_all(&self.dir)?;
            write_downscaled(&bytes, &path)?;
        }
        Ok(key)
    }
}

/// Decodes the picture in `bytes`, shrinks it to fit [`MAX_SIDE`], and writes it to `path` as
/// JPEG. Written to a temporary file first, so a crash never leaves half a cover under the key.
fn write_downscaled(bytes: &[u8], path: &Path) -> io::Result<()> {
    let image = ImageReader::new(io::Cursor::new(bytes))
        .with_guessed_format()?
        .decode()
        .map_err(io::Error::other)?;
    let image = if image.width().max(image.height()) > MAX_SIDE {
        image.thumbnail(MAX_SIDE, MAX_SIDE)
    } else {
        image
    };
    let partial = path.with_extension("part");
    let file = io::BufWriter::new(fs::File::create(&partial)?);
    JpegEncoder::new_with_quality(file, JPEG_QUALITY)
        .encode_image(&image.into_rgb8())
        .map_err(io::Error::other)?;
    fs::rename(&partial, path)
}

#[cfg(test)]
mod tests {
    use image::{ImageFormat, RgbImage};

    use super::*;
    use crate::opening::scratch::scratch_dir;

    fn picture(path: &Path, width: u32, height: u32) {
        RgbImage::from_pixel(width, height, image::Rgb([200, 40, 40]))
            .save_with_format(path, ImageFormat::Png)
            .expect("write a picture");
    }

    #[test]
    fn covers_are_downscaled_and_shared_by_content() {
        let dir = scratch_dir();
        let cache = ArtworkCache::new(dir.join("cache"));
        let (big, copy) = (dir.join("big.png"), dir.join("copy.png"));
        picture(&big, 1800, 1200);
        fs::copy(&big, &copy).expect("copy");
        let key = cache.store(&big).expect("cached");
        assert_eq!(cache.store(&copy), Some(key.clone()));
        let cached = image::open(cache.path(&key)).expect("a JPEG");
        assert_eq!((cached.width(), cached.height()), (MAX_SIDE, 400));
    }

    #[test]
    fn small_covers_keep_their_size_and_non_pictures_are_refused() {
        let dir = scratch_dir();
        let cache = ArtworkCache::new(dir.join("cache"));
        let small = dir.join("small.png");
        picture(&small, 300, 300);
        let key = cache.store(&small).expect("cached");
        assert_eq!(image::open(cache.path(&key)).expect("a JPEG").width(), 300);
        let text = dir.join("cover.jpg");
        fs::write(&text, "not a picture").expect("write");
        assert_eq!(cache.store(&text), None);
        assert_eq!(cache.store(&dir.join("missing.jpg")), None);
    }
}
