//! Music mode's cover (S8.1.1, S8.1.3): where the playing file's cover image is, and the blurred
//! copy behind it.
//!
//! The library's cover comes first, read on the history thread. A file the library hasn't read
//! gets what the indexer would find (`extract::choose_artwork`): the picture inside it, read by
//! the tag reader, else an image in its folder. Either is saved in the library's cover cache, downscaled, so the large tile
//! never decodes a 20 MB scan. The backdrop is that cover shrunk to [`BACKDROP_SIDE`] pixels and
//! blurred once, which the view stretches over the content area: a cheap copy, not a live blur.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::channel::oneshot;
use gpui::{App, AppContext as _, RenderImage, Task};
use image::{Frame, ImageReader, imageops};
use rusqlite::Connection;

use crate::db::metadata::MediaKind;
use crate::db::{artwork, media};
use crate::engine::tags::{self, Lookup as TagLookup, Probe};
use crate::history::History;
use crate::library::artwork_cache::ArtworkCache;
use crate::library::{self, extract, sources};
use crate::settings::{self, stored_path};

/// The backdrop's longest side before blurring, in pixels: stretched over a window, nothing
/// finer would show through the blur.
const BACKDROP_SIDE: u32 = 128;
/// The blur's standard deviation, in the backdrop's pixels.
const BACKDROP_BLUR: f32 = 6.;

/// A cover found for a file.
#[derive(Clone)]
pub(super) struct Cover {
    /// The cached image, at most `artwork_cache::MAX_SIDE` pixels on a side.
    pub path: PathBuf,
    /// The blurred copy behind it, if the image could be decoded.
    pub backdrop: Option<Arc<RenderImage>>,
}

/// Where a file's cover lookup stands.
#[derive(Clone, Default)]
pub(super) enum Lookup {
    /// Still looking: nothing is drawn yet, so a song with a cover never flashes the
    /// placeholder.
    #[default]
    Looking,
    /// Found.
    Found(Cover),
    /// It has none: the placeholder tile shows.
    Missing,
}

/// Looks for the cover of the file at `path`, off the UI thread.
pub(super) fn find(path: PathBuf, cx: &App) -> Task<Option<Cover>> {
    let Some(cache) = library::artwork_cache() else { return Task::ready(None) };
    let lookup = sources::lookup(&settings::get(cx).prefs);
    let known = known_cover(path.clone(), cache.clone(), cx);
    cx.background_spawn(async move {
        let found = match known.await {
            Some(found) => found,
            None => cover_of_unknown(&path, &cache, lookup)?,
        };
        let backdrop = backdrop(&found);
        Some(Cover { path: found, backdrop })
    })
}

/// The cover the library keeps for the file at `path`, read on the history thread.
fn known_cover(
    path: PathBuf,
    cache: ArtworkCache,
    cx: &App,
) -> impl Future<Output = Option<PathBuf>> + use<> {
    let (reply, answer) = oneshot::channel();
    match History::get(cx) {
        Some(history) => history.with_database(move |connection| {
            let key = library_cover(connection, &path).unwrap_or_else(|error| {
                tracing::debug!("music mode: can't read the cover: {error}");
                None
            });
            let _ = reply.send(key.map(|key| cache.path(&key)).filter(|cached| cached.is_file()));
        }),
        None => drop(reply.send(None)),
    }
    async move { answer.await.ok().flatten() }
}

/// The cache key of the cover the library has for the file at `path`.
fn library_cover(connection: &Connection, path: &Path) -> rusqlite::Result<Option<String>> {
    let Some(located) = media::find_at(connection, &stored_path::encode(path))? else {
        return Ok(None);
    };
    Ok(artwork::of_media(connection, located.id)?.map(|art| art.cache_key))
}

/// Finds and caches a cover for a file the library hasn't read, as the indexer would: the
/// picture inside it, else an image in its folder unless `lookup` keeps to the file. Blocks.
fn cover_of_unknown(path: &Path, cache: &ArtworkCache, lookup: TagLookup) -> Option<PathBuf> {
    let found = match tags::default_reader().read(path, lookup) {
        Probe::Read(probed) => probed.artwork,
        Probe::Unreadable | Probe::Unavailable => None,
    };
    let (source, _) = extract::choose_artwork(found.as_deref(), path, MediaKind::Audio, lookup)?;
    cache.store(&source).map(|key| cache.path(&key))
}

/// The image at `path` shrunk and blurred, as GPUI draws it. Blocks on decoding.
fn backdrop(path: &Path) -> Option<Arc<RenderImage>> {
    let image = ImageReader::open(path).ok()?.with_guessed_format().ok()?.decode().ok()?;
    let small = image.thumbnail(BACKDROP_SIDE, BACKDROP_SIDE).to_rgba8();
    let mut blurred = imageops::blur(&small, BACKDROP_BLUR);
    // GPUI takes BGRA bytes in an `RgbaImage`.
    for pixel in blurred.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Some(Arc::new(RenderImage::new([Frame::new(blurred)])))
}

#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};

    use super::*;

    /// A file named `name` in a folder of this test's own, removed first if it was left over.
    fn scratch_file(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rvlc-music-cover-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a temporary folder");
        dir.join(name)
    }

    #[test]
    fn the_backdrop_is_a_small_copy() {
        let path = scratch_file("cover.png");
        RgbaImage::from_pixel(600, 300, Rgba([200, 40, 10, 255])).save(&path).expect("saved");
        let image = backdrop(&path).expect("a decodable cover has a backdrop");
        let size = image.size(0);
        assert_eq!((size.width.0, size.height.0), (128, 64));
        // Red and blue swap places: GPUI reads the bytes as BGRA.
        let bytes = image.as_bytes(0).expect("one frame");
        assert_eq!(&bytes[..3], &[10, 40, 200]);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn a_file_that_is_no_picture_has_no_backdrop() {
        let path = scratch_file("broken.jpg");
        std::fs::write(&path, b"not a picture").expect("written");
        assert!(backdrop(&path).is_none());
        std::fs::remove_file(path).ok();
    }
}
