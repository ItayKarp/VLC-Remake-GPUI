//! From Link (S6.4.5): downloading a subtitle file from an `http` or `https` link into the
//! app's own `subtitles/` folder, where it stays for the media it is linked to.
//!
//! The file keeps the name the link gives it, in a folder named after the link,
//! so two links to `English.srt` don't overwrite each other and the same link downloads into the
//! same place again. It must have a subtitle extension and be what that says (S6.1's checks);
//! anything else is deleted and refused. A gzip or zip download is unpacked first ([`unpack`]).
//! OpenSubtitles downloads ([`super::opensubtitles`]) are stored the same way.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use url::Url;
use xxhash_rust::xxh3::xxh3_64;

use super::unpack;
use crate::engine::sidecar;

/// How long a download may take, connecting included.
pub(super) const TIMEOUT: Duration = Duration::from_secs(30);
/// The most a downloaded file may hold: S6.1's limit for text subtitles.
pub(super) const MAX_BYTES: u64 = 32 * 1024 * 1024;

/// Why a link didn't give a subtitle file, in words the dialog shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The text isn't an `http` or `https` link.
    NotALink,
    /// The link's name isn't a subtitle file's (`.srt`, `.ass`, …).
    NotSubtitleName,
    /// The server answered with an error status.
    Status(u16),
    /// The file is bigger than a subtitle file can be.
    TooBig,
    /// What came back isn't what its name says.
    NotSubtitles,
    /// What came back holds no text.
    Empty,
    /// The server couldn't be reached, or the connection failed; why, in words.
    Network(String),
    /// The file couldn't be saved; why, in words.
    Disk(String),
}

impl Failure {
    /// What the dialog says about it.
    pub fn message(&self) -> String {
        match self {
            Self::NotALink => "Enter a link that starts with http:// or https://.".to_owned(),
            Self::NotSubtitleName => {
                "The link doesn't name a subtitle file (.srt, .ass, .vtt, …).".to_owned()
            }
            Self::Status(404) => "Nothing was found at that link.".to_owned(),
            Self::Status(code) => format!("The server refused the download (HTTP {code})."),
            Self::TooBig => "That file is too big to be subtitles.".to_owned(),
            Self::NotSubtitles => "That file isn't subtitles.".to_owned(),
            Self::Empty => "That subtitle file is empty.".to_owned(),
            Self::Network(why) => format!("Couldn't download it: {why}."),
            Self::Disk(why) => format!("Couldn't save it: {why}."),
        }
    }
}

/// Reads `text` as a link to download: `http` or `https`, with a host.
pub fn parse_link(text: &str) -> Result<Url, Failure> {
    let url = Url::parse(text.trim()).map_err(|_| Failure::NotALink)?;
    let web = matches!(url.scheme(), "http" | "https");
    if web && url.host_str().is_some_and(|host| !host.is_empty()) {
        Ok(url)
    } else {
        Err(Failure::NotALink)
    }
}

/// The file name the link gives: its last path segment, decoded.
pub fn name_of(url: &Url) -> Option<String> {
    let segment = url.path_segments()?.rfind(|segment| !segment.is_empty())?;
    let name = percent_decode(segment);
    let wanted = !name.contains(['/', '\\']) && !name.starts_with('.');
    (wanted && sidecar::has_subtitle_extension(Path::new(&name))).then_some(name)
}

/// Downloads the subtitle file `url` links to into `folder` (the app's `subtitles/`), through
/// `proxy` when one is set. Blocks, so it runs off the UI thread.
pub fn download(url: &Url, folder: &Path, proxy: Option<&Url>) -> Result<PathBuf, Failure> {
    let name = name_of(url).ok_or(Failure::NotSubtitleName)?;
    let bytes = fetch(url, proxy)?;
    let destination = folder.join(format!("{:016x}", xxh3_64(url.as_str().as_bytes())));
    store(&destination, &name, bytes)
}

/// Saves the downloaded `bytes`, unpacked when they are a gzip or zip archive, as `name` in
/// the new folder `destination`, and checks they are what the name says (S6.1). Anything else
/// is deleted and refused.
pub(super) fn store(destination: &Path, name: &str, bytes: Vec<u8>) -> Result<PathBuf, Failure> {
    let (name, bytes) = unpack::unpack(name, bytes)?;
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Err(Failure::Empty);
    }
    fs::create_dir_all(destination).map_err(|error| Failure::Disk(error.to_string()))?;
    let path = destination.join(name);
    fs::write(&path, bytes).map_err(|error| Failure::Disk(error.to_string()))?;
    if sidecar::is_subtitle_file(&path) {
        return Ok(path);
    }
    // Nothing links to it, and it isn't subtitles.
    let _ = fs::remove_dir_all(destination);
    Err(Failure::NotSubtitles)
}

/// The proxy to download through: `proxy` when one is set, else the environment's.
pub(super) fn proxy_for(proxy: Option<&Url>) -> Result<Option<ureq::Proxy>, Failure> {
    match proxy {
        Some(proxy) => ureq::Proxy::new(proxy.as_str()).map(Some).map_err(|error| network(&error)),
        None => Ok(ureq::Proxy::try_from_env()),
    }
}

/// Reads a response's body, at most [`MAX_BYTES`] of it.
pub(super) fn read_body(body: &mut ureq::Body) -> Result<Vec<u8>, Failure> {
    body.with_config().limit(MAX_BYTES).read_to_vec().map_err(|error| match error {
        ureq::Error::BodyExceedsLimit(_) => Failure::TooBig,
        other => network(&other),
    })
}

/// The body `url` answers with, at most [`MAX_BYTES`] of it.
fn fetch(url: &Url, proxy: Option<&Url>) -> Result<Vec<u8>, Failure> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .proxy(proxy_for(proxy)?)
        .build()
        .into();
    let mut response = agent.get(url.as_str()).call().map_err(|error| match error {
        ureq::Error::StatusCode(code) => Failure::Status(code),
        other => network(&other),
    })?;
    read_body(response.body_mut())
}

pub(super) fn network(error: &ureq::Error) -> Failure {
    Failure::Network(error.to_string())
}

/// `text` with its `%XX` escapes decoded; an escape that isn't valid UTF-8 stays as it is.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let escape = (bytes[index] == b'%')
            .then(|| text.get(index + 1..index + 3))
            .flatten()
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        if let Some(byte) = escape {
            decoded.push(byte);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).unwrap_or_else(|_| text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_web_links_are_taken() {
        assert!(parse_link(" https://example.com/a.srt ").is_ok());
        assert_eq!(parse_link("ftp://example.com/a.srt"), Err(Failure::NotALink));
        assert_eq!(parse_link("example.com/a.srt"), Err(Failure::NotALink));
    }

    #[test]
    fn the_name_is_the_last_segment_decoded() {
        let url = Url::parse("https://example.com/subs/The%20Wand%20S01E01.en.srt?x=1").unwrap();
        assert_eq!(name_of(&url).as_deref(), Some("The Wand S01E01.en.srt"));
        let page = Url::parse("https://example.com/subtitles/12345").unwrap();
        assert_eq!(name_of(&page), None);
    }

    #[test]
    fn a_bad_escape_stays_as_written() {
        assert_eq!(percent_decode("a%2"), "a%2");
        assert_eq!(percent_decode("%E2%9C%93"), "✓");
    }
}
