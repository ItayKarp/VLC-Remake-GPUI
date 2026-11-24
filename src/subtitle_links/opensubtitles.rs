//! From OpenSubtitles (S6.4.4, S6.5): what the Add Subtitles dialog asks of OpenSubtitles.com,
//! a search whose results it lists to pick from, then downloading the picked ones on Save.
//!
//! A search sends the video's moviehash ([`moviehash`]) and the words in the search field, so
//! subtitles made for that very file come first, then the title's (S6.5.2); it runs only once
//! the user has allowed it (S6.5.4, Settings → Privacy). A download asks [`api`] for the file's
//! link, signed in when an account is set ([`account`]), and keeps the file in the app's
//! `subtitles/` folder, unpacked and checked as From Link's are. Every call blocks on the
//! network, so it runs off the UI thread.

mod account;
mod api;
mod failure;
mod moviehash;
mod results;

use std::path::{Path, PathBuf};

use url::Url;

use self::api::Client;
pub use self::failure::Failure;
use crate::engine::language::Language;
use crate::engine::sidecar;
use crate::subtitle_links::download;

/// The extension a download gets when its name has none: the API's default format.
const DEFAULT_EXTENSION: &str = "srt";

/// One subtitle file OpenSubtitles found.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    /// OpenSubtitles' id of the file, to download it by.
    pub id: String,
    /// The file's name.
    pub name: String,
    /// The release the subtitles were made for ("The.Wand.S01E01.720p.WEB"); may be empty.
    pub release: String,
    /// The API's language code (`en`, `pt-BR`).
    pub language: String,
    /// The rating out of 10, when someone rated it.
    pub rating: Option<f32>,
    /// How often it was downloaded.
    pub downloads: u64,
    /// Whether it was made for this very file (its moviehash matched).
    pub matches_file: bool,
}

impl Found {
    /// What the results list names it by: the release, else the file name.
    pub fn title(&self) -> &str {
        if self.release.trim().is_empty() { &self.name } else { &self.release }
    }

    /// Its language, rating, downloads and whether it fits the file, for its row
    /// ("English · ★ 8.5 · 1,204 downloads · Made for this file").
    pub fn details(&self) -> String {
        let mut parts = vec![results::language_name(&self.language)];
        parts.extend(
            self.rating.map(|rating| format!("★ {}", crate::i18n::decimal(f64::from(rating), 1))),
        );
        parts.push(trn!("{count} download", "{count} downloads", self.downloads));
        if self.matches_file {
            parts.push(tr!("Made for this file").to_owned());
        }
        parts.retain(|part| !part.is_empty());
        parts.join(" · ")
    }
}

/// A search: the words, the language, and the video whose hash to send.
#[derive(Clone, Debug, Default)]
pub struct Query {
    /// The search field's words ("The Wand S01E01").
    pub text: String,
    /// The language to find; `None` finds every language.
    pub language: Option<Language>,
    /// The video, to search by its hash first.
    pub video: Option<PathBuf>,
    /// The HTTP proxy setting; `None` uses the environment's.
    pub proxy: Option<Url>,
}

/// Where and how downloads go.
#[derive(Clone, Debug, Default)]
pub struct Downloads {
    /// The app's `subtitles/` folder.
    pub folder: PathBuf,
    /// The OpenSubtitles username in Settings → Privacy; empty for none.
    pub username: String,
    /// The HTTP proxy setting; `None` uses the environment's.
    pub proxy: Option<Url>,
}

/// Searches OpenSubtitles for `query`, best match first.
pub fn search(query: &Query) -> Result<Vec<Found>, Failure> {
    let client = Client::new(query.proxy.as_ref())?;
    let languages = query.language.as_ref().map(results::language_codes);
    let by_hash = match query.video.as_deref().and_then(moviehash::of) {
        Some(hash) => {
            client.search(&results::hash_parameters(&hash, languages.as_deref()))?.into_found()
        }
        None => Vec::new(),
    };
    let by_query = if query.text.trim().is_empty() {
        Vec::new()
    } else {
        client.search(&results::query_parameters(&query.text, languages.as_deref()))?.into_found()
    };
    Ok(results::merge(by_hash, by_query))
}

/// Downloads `found` into `downloads.folder`, answering where it went.
pub fn download(found: &Found, downloads: &Downloads) -> Result<PathBuf, Failure> {
    let client = Client::new(downloads.proxy.as_ref())?;
    let link = link(&client, found, &downloads.username)?;
    let bytes = client.fetch(&link.url)?;
    let name = file_name(link.file_name.as_deref().unwrap_or(&found.name), &found.id);
    let destination = downloads.folder.join("opensubtitles").join(&found.id);
    download::store(&destination, &name, bytes).map_err(Failure::File)
}

/// `found`'s download link, signed in as `username` when it has a saved password; a token
/// OpenSubtitles no longer takes signs in again, once.
fn link(client: &Client, found: &Found, username: &str) -> Result<api::Link, Failure> {
    let token = account::token(client, username)?;
    match client.link(&found.id, token.as_deref()) {
        Err(Failure::Unauthorized) if token.is_some() => {
            account::forget_token();
            let token = account::token(client, username)?;
            client.link(&found.id, token.as_deref())
        }
        answer => answer,
    }
}

/// A safe file name for a download named `name`: its last part, with a subtitle extension.
fn file_name(name: &str, id: &str) -> String {
    let last = name.rsplit(['/', '\\']).next().unwrap_or_default().trim();
    let last = if last.is_empty() || last.starts_with('.') { id } else { last };
    if sidecar::has_subtitle_extension(Path::new(last)) {
        last.to_owned()
    } else {
        format!("{last}.{DEFAULT_EXTENSION}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_says_language_rating_and_downloads() {
        let found = Found {
            id: "1".to_owned(),
            name: "a.srt".to_owned(),
            release: "The.Wand.S01E01".to_owned(),
            language: "en".to_owned(),
            rating: Some(8.46),
            downloads: 12_345,
            matches_file: true,
        };
        assert_eq!(found.title(), "The.Wand.S01E01");
        assert_eq!(found.details(), "English · ★ 8.5 · 12,345 downloads · Made for this file");
        let plain = Found { rating: None, downloads: 1, matches_file: false, ..found };
        assert_eq!(plain.details(), "English · 1 download");
    }

    #[test]
    fn download_names_are_kept_safe() {
        assert_eq!(file_name("../../evil/Movie.en.srt", "7"), "Movie.en.srt");
        assert_eq!(file_name("The Wand", "7"), "The Wand.srt");
        assert_eq!(file_name("", "7"), "7.srt");
    }
}
