//! Reading a media file's tags, tracks and cover without playing it (S4.3.1), for the library's
//! metadata extraction. A [`TagReader`] blocks while it reads, so it runs on the indexer's
//! thread; [`default_reader`] reads with libVLC, and falls back to one that has no backend.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::db::metadata::Metadata;

/// What reading one file found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Probed {
    /// Its tags, kind, codecs and resolution.
    pub metadata: Metadata,
    /// Its length, if the container says.
    pub duration: Option<Duration>,
    /// A cover image file the backend found for it: one it extracted from the file, or an
    /// image next to it.
    pub artwork: Option<PathBuf>,
}

/// How reading a file went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Probe {
    /// It was read.
    Read(Box<Probed>),
    /// It couldn't be parsed (damaged, gone, not media after all, or too slow).
    Unreadable,
    /// There is no backend to read with: nothing should be marked as read.
    Unavailable,
}

/// How a reader may look things up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Lookup {
    /// Only the file: no cover images from its folder (Library → "Fetch titles and artwork",
    /// S5.9.2).
    File,
    /// Only the file and its folder (the default).
    #[default]
    Local,
    /// Also the internet, for titles and cover art (Privacy → "Fetch titles and cover art from
    /// the internet", S4.3.4).
    Online,
}

/// Reads media files' tags. Every call blocks until the file is read or gives up.
pub trait TagReader: Send {
    /// Reads the file at `path`.
    fn read(&mut self, path: &Path, lookup: Lookup) -> Probe;
}

/// The reader to use: libVLC's, which loads libVLC the first time it reads.
pub fn default_reader() -> Box<dyn TagReader> {
    Box::new(super::libvlc::tags::LibVlcTags::default())
}

/// A reader without a backend: every file is [`Probe::Unavailable`].
pub struct NoTags;

impl TagReader for NoTags {
    fn read(&mut self, _: &Path, _: Lookup) -> Probe {
        Probe::Unavailable
    }
}
