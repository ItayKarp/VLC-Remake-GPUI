//! The thumbnail cache (S4.4.2): each picture saved once per [`Variant`] — a list row's square
//! tile and a grid tile, at 1× and 2× — as JPEG in the cache dir, cropped to fill the tile.
//!
//! - A video's pictures are named by its content identity, so a moved or copied file keeps them;
//!   a song's come from its cover and are named by the cover's key, so an album shares them.
//! - The folder is created when needed, so deleting it while the app runs only costs the
//!   thumbnails being made again.
//! - What is cached, and when it was last shown, is in the `thumbnails` table; [`evict`] keeps
//!   the cache under [`MAX_BYTES`] by deleting the least recently shown.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, RgbImage};
use rusqlite::Connection;
use xxhash_rust::xxh3::xxh3_128;

use crate::db::thumbnails::{self, Cached, SizeClass, VARIANTS, Variant};
use crate::engine::thumbnail::Frame;

/// A list row's tile in logical pixels: 32 × 32, drawn 64 × 64 in the 2× designs.
pub const LIST_SIZE: (u32, u32) = (32, 32);
/// A grid tile: 16:9, as in the full library's grid (`player_03`).
pub const GRID_SIZE: (u32, u32) = (320, 180);
/// The most the cache may hold before the least recently shown pictures are deleted.
pub const MAX_BYTES: u64 = 256 * 1024 * 1024;
/// Eviction deletes down to this share of [`MAX_BYTES`], so it doesn't run after every picture.
const EVICT_TO_PERCENT: u64 = 90;
/// How many pictures eviction looks at a time.
const EVICT_BATCH: usize = 64;
/// The quality pictures are saved at.
const JPEG_QUALITY: u8 = 85;
/// The cached files' extension.
const EXTENSION: &str = "jpg";
/// Bytes per pixel of a grabbed frame.
const BGRA: usize = 4;

/// The thumbnail cache in one folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThumbnailCache {
    dir: PathBuf,
}

/// What names a media's pictures in the cache.
#[derive(Clone, Copy, Debug)]
pub enum Source<'a> {
    /// A frame of the video with this identity.
    Video(&'a str),
    /// The cover cached under this artwork key.
    Cover(&'a str),
}

impl ThumbnailCache {
    /// The cache in `dir`, created when the first picture is saved.
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Where the picture cached as `key` is.
    pub fn path(&self, key: &str) -> PathBuf {
        self.dir.join(key).with_extension(EXTENSION)
    }

    /// Saves `picture` in every variant, named for `source`; answers what was saved, or the
    /// error that stopped it.
    pub fn store(&self, source: Source<'_>, picture: &DynamicImage) -> io::Result<Vec<Cached>> {
        fs::create_dir_all(&self.dir)?;
        let base = base_key(source);
        VARIANTS
            .iter()
            .map(|&variant| {
                let cache_key = format!("{base}-{}", variant_name(variant));
                let path = self.path(&cache_key);
                write_jpeg(&fill(picture, variant), &path)?;
                Ok(Cached { variant, cache_key, bytes: fs::metadata(&path)?.len() })
            })
            .collect()
    }

    /// Whether every picture in `images` is still on the disk.
    pub fn has_all(&self, images: &[Cached]) -> bool {
        images.len() == VARIANTS.len()
            && images.iter().all(|image| self.path(&image.cache_key).is_file())
    }

    /// Deletes the least recently shown pictures while the cache holds more than `max_bytes`
    /// ([`MAX_BYTES`]), down to [`EVICT_TO_PERCENT`] of it.
    pub fn evict(&self, connection: &Connection, max_bytes: u64) -> rusqlite::Result<()> {
        let mut total = thumbnails::total_bytes(connection)?;
        if total <= max_bytes {
            return Ok(());
        }
        let target = max_bytes / 100 * EVICT_TO_PERCENT;
        while total > target {
            let oldest = thumbnails::least_recent(connection, EVICT_BATCH)?;
            if oldest.is_empty() {
                break;
            }
            total = self.delete_down_to(connection, &oldest, total, target)?;
        }
        Ok(())
    }
}

impl ThumbnailCache {
    /// Deletes `oldest` in order while the cache holds more than `target`; answers its new size.
    fn delete_down_to(
        &self,
        connection: &Connection,
        oldest: &[(String, u64)],
        mut total: u64,
        target: u64,
    ) -> rusqlite::Result<u64> {
        for (key, bytes) in oldest {
            if total <= target {
                break;
            }
            // A file already gone is fine: only its row is left to delete.
            let _ = fs::remove_file(self.path(key));
            thumbnails::evict(connection, key)?;
            total = total.saturating_sub(*bytes);
        }
        Ok(total)
    }
}

/// The size of `variant`'s picture in pixels.
pub fn pixel_size(variant: Variant) -> (u32, u32) {
    let (width, height) = match variant.class {
        SizeClass::List => LIST_SIZE,
        SizeClass::Grid => GRID_SIZE,
    };
    let scale = u32::from(variant.scale);
    (width * scale, height * scale)
}

/// A grabbed BGRA frame as a picture.
pub fn frame_picture(frame: &Frame) -> Option<DynamicImage> {
    let rgb: Vec<u8> = frame
        .bgra
        .as_chunks::<BGRA>()
        .0
        .iter()
        .flat_map(|pixel| [pixel[2], pixel[1], pixel[0]])
        .collect();
    RgbImage::from_raw(frame.width, frame.height, rgb).map(DynamicImage::ImageRgb8)
}

/// The part of every cache key a media's pictures share.
fn base_key(source: Source<'_>) -> String {
    match source {
        Source::Video(identity) => format!("v{:032x}", xxh3_128(identity.as_bytes())),
        Source::Cover(artwork) => format!("a{artwork}"),
    }
}

fn variant_name(variant: Variant) -> String {
    format!("{}@{}x", variant.class.as_str(), variant.scale)
}

/// `picture` scaled to cover `variant`'s size and cropped to it, centered.
fn fill(picture: &DynamicImage, variant: Variant) -> RgbImage {
    let (width, height) = pixel_size(variant);
    picture.resize_to_fill(width, height, FilterType::Triangle).into_rgb8()
}

/// Writes `picture` to `path` as JPEG, through a temporary file so a crash never leaves half a
/// picture under the key.
fn write_jpeg(picture: &RgbImage, path: &Path) -> io::Result<()> {
    let partial = path.with_extension("part");
    let file = io::BufWriter::new(fs::File::create(&partial)?);
    JpegEncoder::new_with_quality(file, JPEG_QUALITY)
        .encode_image(picture)
        .map_err(io::Error::other)?;
    fs::rename(&partial, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opening::scratch::scratch_dir;

    fn frame(width: u32, height: u32) -> Frame {
        let bgra = [10, 20, 200, 255].repeat((width * height) as usize);
        Frame { width, height, bgra }
    }

    #[test]
    fn every_variant_is_cropped_to_its_tile() {
        let cache = ThumbnailCache::new(scratch_dir().join("thumbs"));
        let picture = frame_picture(&frame(640, 272)).expect("a picture");
        let stored = cache.store(Source::Video("id"), &picture).expect("stored");
        assert!(cache.has_all(&stored));
        for image in &stored {
            let saved = image::open(cache.path(&image.cache_key)).expect("a JPEG");
            assert_eq!((saved.width(), saved.height()), pixel_size(image.variant));
            assert!(image.bytes > 0);
        }
        // BGRA becomes RGB: the frame is red.
        let pixel = image::open(cache.path(&stored[0].cache_key)).expect("a JPEG").into_rgb8();
        assert!(pixel.get_pixel(0, 0)[0] > 150);
    }

    #[test]
    fn videos_and_covers_are_named_apart_and_a_deleted_folder_comes_back() {
        let dir = scratch_dir().join("thumbs");
        let cache = ThumbnailCache::new(dir.clone());
        let picture = frame_picture(&frame(8, 8)).expect("a picture");
        let video = cache.store(Source::Video("same"), &picture).expect("stored");
        fs::remove_dir_all(&dir).expect("delete the cache");
        assert!(!cache.has_all(&video));
        let cover = cache.store(Source::Cover("same"), &picture).expect("stored again");
        assert_ne!(video[0].cache_key, cover[0].cache_key);
        assert!(cache.has_all(&cover));
    }

    #[test]
    fn eviction_deletes_the_least_recently_shown() {
        use crate::db::Database;
        use crate::db::media::{self, Entry, MediaId};

        let cache = ThumbnailCache::new(scratch_dir().join("thumbs"));
        let picture = frame_picture(&frame(64, 36)).expect("a picture");
        let mut database = Database::in_memory();
        let c = database.connection();
        let mut stored = Vec::new();
        for (at, identity) in ["old", "new"].into_iter().enumerate() {
            let entry = Entry { identity: identity.into(), location: identity.into(), stamp: None };
            let id: MediaId = media::insert(c, &entry).expect("media");
            let images = cache.store(Source::Video(identity), &picture).expect("stored");
            thumbnails::record(c, id, &images, at as i64).expect("recorded");
            stored.push(images);
        }
        let total = thumbnails::total_bytes(c).expect("total");
        cache.evict(c, total).expect("nothing to evict");
        assert!(cache.has_all(&stored[0]));
        cache.evict(c, total - 1).expect("evicted");
        assert!(!cache.has_all(&stored[0]) && cache.has_all(&stored[1]));
    }

    #[test]
    fn a_short_frame_is_no_picture() {
        let broken = Frame { width: 4, height: 4, bgra: vec![0; 8] };
        assert!(frame_picture(&broken).is_none());
    }
}
