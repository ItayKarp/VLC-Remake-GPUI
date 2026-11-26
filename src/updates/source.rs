//! Where updates come from (S9.5.1): the signed manifest, and downloading a package it lists.
//! Both are blocking HTTPS requests, run off the UI thread, through the HTTP proxy in Settings →
//! Network when one is set (the environment's otherwise).
//!
//! The manifest is `update-manifest.json` of the latest GitHub release, its signature beside it
//! in `update-manifest.json.sig`. `RVLC_UPDATE_URL`, when the app is built or runs, names
//! another manifest; its signature is always the same address with `.sig` added.
//!
//! The check depends on [`UpdateSource`], not on HTTPS: [`Https`] is the source the app uses, and
//! tests stand in one that serves a manifest and packages without a network.

use std::fs::File;
use std::io::{self, BufWriter};
use std::path::Path;
use std::time::Duration;

use url::Url;

use super::manifest::{self, Invalid, Manifest, Package};
use super::signing::{self, HashingWriter, Untrusted};

/// The variable naming another manifest, read when the app is built and when it runs.
pub const URL_VARIABLE: &str = "RVLC_UPDATE_URL";
/// The manifest of the latest release.
const DEFAULT_URL: &str =
    "https://github.com/ItayKarp/VLC-Remake-GPUI/releases/latest/download/update-manifest.json";
/// What the signature's address adds to the manifest's.
const SIGNATURE_SUFFIX: &str = ".sig";
/// How long reading the manifest or its signature may take, connecting included.
const MANIFEST_TIMEOUT: Duration = Duration::from_secs(30);
/// The most a manifest or signature may hold.
const MAX_MANIFEST_BYTES: u64 = 256 * 1024;
/// How long a package's download may take: an hour, for slow connections.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_hours(1);
/// Names the app to the server.
const USER_AGENT: &str = concat!("RedesignedVLC/", env!("CARGO_PKG_VERSION"));

/// Why an update couldn't be checked for or downloaded. Logged, never shown: the next check
/// tries again.
#[derive(Debug)]
pub enum Failure {
    /// The server couldn't be reached, or answered with an error; why.
    Network(String),
    /// The manifest's signature isn't trusted.
    Untrusted(Untrusted),
    /// The manifest can't be used.
    Invalid(Invalid),
    /// The download isn't the package the manifest describes.
    Corrupt,
    /// Saving the download failed.
    Disk(io::Error),
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network(why) => write!(f, "network: {why}"),
            Self::Untrusted(why) => write!(f, "the manifest's signature isn't trusted: {why:?}"),
            Self::Invalid(why) => write!(f, "the manifest can't be used: {why:?}"),
            Self::Corrupt => write!(f, "the download doesn't match the manifest"),
            Self::Disk(error) => write!(f, "couldn't save the download: {error}"),
        }
    }
}

/// Where updates come from: the newest version's manifest, and the packages it lists.
pub trait UpdateSource: Send {
    /// Where the manifest is read from, for the log.
    fn location(&self) -> &str;
    /// The manifest, once its signature is trusted. Blocks.
    fn manifest(&self) -> Result<Manifest, Failure>;
    /// Downloads `package` to `path`; the file is left only if it is the package the manifest
    /// describes. Blocks.
    fn download(&self, package: &Package, path: &Path) -> Result<(), Failure>;
}

/// The release's manifest and packages over HTTPS, through the proxy in Settings → Network (the
/// environment's when `None`).
pub struct Https {
    url: String,
    proxy: Option<Url>,
}

impl Https {
    /// The source at [`manifest_url`], reached through `proxy`.
    pub fn new(proxy: Option<Url>) -> Self {
        Self { url: manifest_url(), proxy }
    }
}

impl UpdateSource for Https {
    fn location(&self) -> &str {
        &self.url
    }

    fn manifest(&self) -> Result<Manifest, Failure> {
        fetch_manifest(&self.url, self.proxy.as_ref())
    }

    fn download(&self, package: &Package, path: &Path) -> Result<(), Failure> {
        download(package, path, self.proxy.as_ref())
    }
}

/// The manifest's address: the running app's `RVLC_UPDATE_URL`, else the one it was built
/// with, else the latest GitHub release's.
pub fn manifest_url() -> String {
    let at_run_time = std::env::var(URL_VARIABLE).ok();
    let built_with = option_env!("RVLC_UPDATE_URL").map(str::to_owned);
    at_run_time
        .into_iter()
        .chain(built_with)
        .find(|url| !url.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_URL.to_owned())
}

/// Downloads the manifest at `url` and its signature, and reads it once the signature is
/// trusted. Blocks.
fn fetch_manifest(url: &str, proxy: Option<&Url>) -> Result<Manifest, Failure> {
    let parsed = Url::parse(url).map_err(|error| Failure::Network(error.to_string()))?;
    if !manifest::is_secure(&parsed) {
        return Err(Failure::Invalid(Invalid::Insecure(parsed)));
    }
    let agent = agent(proxy, MANIFEST_TIMEOUT)?;
    let json = read_small(&agent, url)?;
    let signature = read_small(&agent, &format!("{url}{SIGNATURE_SUFFIX}"))?;
    let signature = String::from_utf8_lossy(&signature);
    signing::verify(&json, &signature, signing::public_key().as_deref())
        .map_err(Failure::Untrusted)?;
    Manifest::parse(&json).map_err(Failure::Invalid)
}

/// Downloads `package` to `path`, checking its size and hash as it arrives; the file is removed
/// unless it is the package. Blocks.
fn download(package: &Package, path: &Path, proxy: Option<&Url>) -> Result<(), Failure> {
    let saved = download_unchecked(package, path, proxy);
    if saved.is_err() {
        // Nothing to keep: a part, or the wrong file.
        let _ = std::fs::remove_file(path);
    }
    saved
}

fn download_unchecked(package: &Package, path: &Path, proxy: Option<&Url>) -> Result<(), Failure> {
    let agent = agent(proxy, DOWNLOAD_TIMEOUT)?;
    let mut response = agent
        .get(package.url.as_str())
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|error| Failure::Network(error.to_string()))?;
    let mut writer = HashingWriter::new(BufWriter::new(File::create(path).map_err(Failure::Disk)?));
    let mut body = response.body_mut().with_config().limit(package.size + 1).reader();
    let copied = io::copy(&mut body, &mut writer).map_err(Failure::Disk)?;
    let (hash, file) = writer.finish();
    let file = file.into_inner().map_err(|error| Failure::Disk(error.into_error()))?;
    file.sync_all().map_err(Failure::Disk)?;
    let fits = copied == package.size && hash.eq_ignore_ascii_case(package.sha256.trim());
    fits.then_some(()).ok_or(Failure::Corrupt)
}

/// A client through `proxy` (the environment's when `None`) that gives up after `timeout`.
fn agent(proxy: Option<&Url>, timeout: Duration) -> Result<ureq::Agent, Failure> {
    let proxy = match proxy {
        Some(proxy) => Some(
            ureq::Proxy::new(proxy.as_str())
                .map_err(|error| Failure::Network(error.to_string()))?,
        ),
        None => ureq::Proxy::try_from_env(),
    };
    Ok(ureq::Agent::config_builder().timeout_global(Some(timeout)).proxy(proxy).build().into())
}

/// The body at `url`, at most [`MAX_MANIFEST_BYTES`].
fn read_small(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>, Failure> {
    let network = |error: ureq::Error| Failure::Network(error.to_string());
    let mut response = agent.get(url).header("User-Agent", USER_AGENT).call().map_err(network)?;
    response.body_mut().with_config().limit(MAX_MANIFEST_BYTES).read_to_vec().map_err(network)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_manifest_is_the_latest_release() {
        assert!(DEFAULT_URL.starts_with("https://"));
        assert!(DEFAULT_URL.ends_with("/releases/latest/download/update-manifest.json"));
    }

    #[test]
    fn insecure_manifests_are_not_fetched() {
        let fetched = fetch_manifest("http://example.com/update-manifest.json", None);
        assert!(matches!(fetched, Err(Failure::Invalid(Invalid::Insecure(_)))));
    }
}
