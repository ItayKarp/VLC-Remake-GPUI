//! Metadata extraction (S4.3): reads the tags, length, codecs, resolution and cover of each media
//! the library found, after the indexer identified it, and writes them to its `media` row.
//!
//! - Media are read a [`BATCH`] at a time, so the indexer answers the watcher in between.
//! - A media is read once ([`metadata::unprobed`]); one that can't be parsed is marked read too,
//!   so a damaged file isn't parsed on every pass. Identifying its file again (Resync, a move)
//!   reads it again.
//! - Online-only files are never read: they have no media row until they are on the disk.
//! - Covers: a picture inside the file first; for songs without one, the folder's cover
//!   ([`folder_art`]). Both are cached downscaled ([`ArtworkCache`]).
//! - Nothing is looked up online unless the user allows it ([`Lookup::Online`], S4.3.4).

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use super::artwork_cache::ArtworkCache;
use super::folder_art;
use super::indexer::store::{Store, call};
use crate::db::artwork::{self, ArtworkSource};
use crate::db::media::MediaId;
use crate::db::metadata::{self, Extracted, MediaKind};
use crate::db::now_ms;
use crate::engine::tags::{Lookup, Probe, Probed, TagReader};
use crate::opening::Cancel;

/// How many media are read before the indexer looks at its commands again.
pub const BATCH: usize = 16;

/// Whether media are still waiting to be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pass {
    /// Every media is read, or reading can't go on now.
    Done,
    /// Run another pass soon.
    More,
}

/// Reads media and writes what it finds.
pub struct Extractor {
    reader: Box<dyn TagReader>,
    cache: Option<ArtworkCache>,
    lookup: Lookup,
}

/// What reading one media found, ready to write.
struct Found {
    extracted: Extracted,
    /// The cached cover's key and where it came from.
    artwork: Option<(String, ArtworkSource)>,
}

impl Extractor {
    /// Reads with `reader` and caches covers in `cache` (no covers without one).
    pub fn new(reader: Box<dyn TagReader>, cache: Option<ArtworkCache>) -> Self {
        Self { reader, cache, lookup: Lookup::Local }
    }

    /// Lets the reader look titles and covers up online, or not, from now on.
    pub fn set_lookup(&mut self, lookup: Lookup) {
        self.lookup = lookup;
    }

    /// Reads the next batch of media and writes what it found.
    pub fn run(&mut self, store: &dyn Store, cancel: &Cancel) -> Pass {
        let batch = call(store, |c| metadata::unprobed(c, MediaId(0), BATCH)).unwrap_or_default();
        let mut results = Vec::with_capacity(batch.len());
        for media in &batch {
            if cancel.is_cancelled() {
                return Pass::Done;
            }
            match self.reader.read(&media.path, self.lookup) {
                Probe::Read(probed) => {
                    results.push((media.id, Some(self.found(*probed, &media.path))));
                }
                Probe::Unreadable => results.push((media.id, None)),
                Probe::Unavailable => break,
            }
        }
        let complete = !batch.is_empty() && results.len() == batch.len();
        let written = !results.is_empty() && call(store, move |c| write(c, &results)).is_some();
        if complete && written { Pass::More } else { Pass::Done }
    }

    /// Prepares what `probed` (read from `path`) found for writing, caching its cover.
    fn found(&self, probed: Probed, path: &Path) -> Found {
        let kind = probed.metadata.kind;
        let artwork = self.cache.as_ref().and_then(|cache| {
            let (image, source) = choose_artwork(probed.artwork.as_deref(), path, kind)?;
            Some((cache.store(&image)?, source))
        });
        let extracted =
            Extracted { metadata: probed.metadata, duration: probed.duration, artwork: None };
        Found { extracted, artwork }
    }
}

/// Which cover a media at `path` gets: the one the reader found inside it (which it saved
/// outside the media's folder), or for a song, the folder's cover. The reader's pick from the
/// folder is ignored, so every song in a folder gets the same, predictable cover.
pub fn choose_artwork(
    found: Option<&Path>,
    path: &Path,
    kind: MediaKind,
) -> Option<(PathBuf, ArtworkSource)> {
    let folder = path.parent()?;
    if let Some(found) = found.filter(|found| found.parent() != Some(folder)) {
        return Some((found.to_path_buf(), ArtworkSource::Embedded));
    }
    if kind != MediaKind::Audio {
        return None;
    }
    folder_art::find(folder).map(|cover| (cover, ArtworkSource::Folder))
}

/// Writes `results` in one transaction: what was found, or that a media couldn't be read.
fn write(
    connection: &mut Connection,
    results: &[(MediaId, Option<Found>)],
) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    let now = now_ms();
    for (id, found) in results {
        let Some(found) = found else {
            metadata::set_probed(&transaction, *id, now)?;
            continue;
        };
        let artwork = match &found.artwork {
            Some((key, source)) => Some(artwork::ensure(&transaction, key, *source)?),
            None => None,
        };
        let extracted = Extracted { artwork, ..found.extracted.clone() };
        metadata::set_extracted(&transaction, *id, &extracted, now)?;
    }
    transaction.commit()
}

#[cfg(test)]
mod tests;
