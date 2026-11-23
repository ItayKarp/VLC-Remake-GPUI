//! From OpenSubtitles (S6.4.4): what the Add Subtitles dialog asks of OpenSubtitles, a search by
//! title and language whose results it lists to pick from, then downloading the picked ones on
//! Save.
//!
//! The REST client comes with S6.5; until then a search answers [`Unavailable`], which the
//! dialog says in place of results.

use std::path::PathBuf;

use crate::engine::language::Language;

/// One subtitle OpenSubtitles found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    /// OpenSubtitles' id of the file, to download it by.
    pub id: String,
    /// The file's name, as the results list it.
    pub name: String,
}

/// OpenSubtitles can't be reached from this build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unavailable;

impl Unavailable {
    /// What the dialog says instead of results.
    pub const MESSAGE: &str = "Searching OpenSubtitles isn't available in this version yet.";
}

/// Searches for `query` in `language` (any language with `None`). Blocks on the network once
/// it has a client, so it runs off the UI thread.
pub fn search(query: &str, language: Option<&Language>) -> Result<Vec<Found>, Unavailable> {
    tracing::debug!(
        "opensubtitles: no client to search {query:?} in {:?}",
        language.map(Language::code)
    );
    Err(Unavailable)
}

/// Downloads `found` into the app's `subtitles/` folder, answering where it went.
pub fn download(found: &Found) -> Result<PathBuf, Unavailable> {
    tracing::debug!("opensubtitles: no client to download {}", found.id);
    Err(Unavailable)
}
