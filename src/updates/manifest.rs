//! The update manifest (S9.5.1): the newest version, where its release notes are, and a package
//! for each way the app is installed, each with its size and SHA-256.
//!
//! The manifest is JSON, signed as a whole (see [`super::signing`]), so the packages' hashes
//! are as trustworthy as its signature:
//!
//! ```json
//! {
//!   "version": "0.2.0",
//!   "notes_url": "https://github.com/…/releases/tag/v0.2.0",
//!   "packages": [
//!     { "os": "macos", "arch": "aarch64", "kind": "app", "url": "https://…/….app.zip",
//!       "size": 36700160, "sha256": "…64 hex digits…" }
//!   ]
//! }
//! ```
//!
//! `os` and `arch` are Rust's names (`std::env::consts`); a macOS package for `universal` runs
//! on either architecture.

use semver::Version;
use serde::{Deserialize, Serialize};
use url::Url;

/// The architecture name of a macOS package that runs on both.
pub const UNIVERSAL: &str = "universal";

/// What the manifest says about the newest version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// The newest version, as Cargo.toml writes it.
    pub version: Version,
    /// The page with its release notes, which also offers its downloads.
    pub notes_url: Url,
    /// One package per kind of installation it can update.
    #[serde(default)]
    pub packages: Vec<Package>,
}

/// One downloadable package of the newest version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Package {
    /// The OS it is for: `macos`, `windows` or `linux`.
    pub os: String,
    /// The architecture it is for, or [`UNIVERSAL`].
    pub arch: String,
    /// How it installs.
    pub kind: PackageKind,
    /// Where to download it.
    pub url: Url,
    /// Its size in bytes.
    pub size: u64,
    /// Its SHA-256, in lowercase hex.
    pub sha256: String,
}

/// How a package installs, which follows from how the running app was installed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageKind {
    /// A zipped macOS app bundle, which takes the place of the running one.
    App,
    /// A Windows Installer package, which upgrades the installed app.
    Msi,
    /// A Linux AppImage, which takes the place of the running one.
    AppImage,
}

/// Why a manifest can't be used.
#[derive(Debug, PartialEq, Eq)]
pub enum Invalid {
    /// It isn't JSON of the right shape; what was wrong.
    Unreadable(String),
    /// A link in it isn't `https` (or, in debug builds, a local `http` server).
    Insecure(Url),
}

impl Manifest {
    /// Reads a manifest from its JSON. Every link must be secure.
    pub fn parse(json: &[u8]) -> Result<Self, Invalid> {
        let manifest: Self =
            serde_json::from_slice(json).map_err(|error| Invalid::Unreadable(error.to_string()))?;
        let links = std::iter::once(&manifest.notes_url)
            .chain(manifest.packages.iter().map(|package| &package.url));
        match links.into_iter().find(|url| !is_secure(url)) {
            Some(url) => Err(Invalid::Insecure(url.clone())),
            None => Ok(manifest),
        }
    }

    /// The package of `kind` for this OS and `arch`: one for exactly `arch` first, then a
    /// universal one.
    pub fn package(&self, kind: PackageKind, os: &str, arch: &str) -> Option<&Package> {
        let fits = |wanted: &str| {
            self.packages
                .iter()
                .find(|package| package.kind == kind && package.os == os && package.arch == wanted)
        };
        fits(arch).or_else(|| fits(UNIVERSAL))
    }

    /// Whether the newest version is worth offering to `running`, unless the user skipped
    /// `skipped` (skipping a version skips the older ones too).
    pub fn is_offered(&self, running: &Version, skipped: Option<&Version>) -> bool {
        self.version > *running && skipped.is_none_or(|skipped| self.version > *skipped)
    }
}

/// The version of the running app.
pub fn running_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).expect("Cargo.toml's version is semver")
}

/// Whether `url` may be trusted to deliver an update: `https`, or a local `http` server in
/// debug builds, for trying updates out.
pub fn is_secure(url: &Url) -> bool {
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    url.scheme() == "https" || (cfg!(debug_assertions) && url.scheme() == "http" && local)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"{
        "version": "0.3.0",
        "notes_url": "https://example.com/releases/v0.3.0",
        "packages": [
            { "os": "macos", "arch": "universal", "kind": "app",
              "url": "https://example.com/u.app.zip", "size": 10, "sha256": "aa" },
            { "os": "macos", "arch": "aarch64", "kind": "app",
              "url": "https://example.com/a.app.zip", "size": 10, "sha256": "bb" },
            { "os": "windows", "arch": "x86_64", "kind": "msi",
              "url": "https://example.com/w.msi", "size": 10, "sha256": "cc" }
        ]
    }"#;

    fn version(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    #[test]
    fn reads_the_manifest_and_picks_this_platforms_package() {
        let manifest = Manifest::parse(MANIFEST.as_bytes()).unwrap();
        assert_eq!(manifest.version, version("0.3.0"));
        let mac = |arch| manifest.package(PackageKind::App, "macos", arch).map(|p| &p.sha256);
        assert_eq!(mac("aarch64").map(String::as_str), Some("bb"));
        assert_eq!(mac("x86_64").map(String::as_str), Some("aa"));
        assert!(manifest.package(PackageKind::Msi, "windows", "aarch64").is_none());
        assert!(manifest.package(PackageKind::AppImage, "linux", "x86_64").is_none());
    }

    #[test]
    fn packages_are_optional() {
        let json = r#"{ "version": "1.0.0", "notes_url": "https://example.com/n" }"#;
        assert_eq!(Manifest::parse(json.as_bytes()).unwrap().packages, []);
    }

    #[test]
    fn insecure_links_and_bad_json_are_refused() {
        let insecure = MANIFEST.replace("https://example.com/w.msi", "http://example.com/w.msi");
        assert!(matches!(Manifest::parse(insecure.as_bytes()), Err(Invalid::Insecure(_))));
        assert!(matches!(Manifest::parse(b"{}"), Err(Invalid::Unreadable(_))));
        let bad_version = MANIFEST.replace("0.3.0\"", "three\"");
        assert!(matches!(Manifest::parse(bad_version.as_bytes()), Err(Invalid::Unreadable(_))));
    }

    #[test]
    fn only_newer_unskipped_versions_are_offered() {
        let manifest = Manifest::parse(MANIFEST.as_bytes()).unwrap();
        assert!(manifest.is_offered(&version("0.2.9"), None));
        assert!(!manifest.is_offered(&version("0.3.0"), None));
        assert!(!manifest.is_offered(&version("0.4.0"), None));
        assert!(!manifest.is_offered(&version("0.2.0"), Some(&version("0.3.0"))));
        assert!(!manifest.is_offered(&version("0.2.0"), Some(&version("0.3.1"))));
        assert!(manifest.is_offered(&version("0.2.0"), Some(&version("0.2.5"))));
    }

    #[test]
    fn the_running_version_parses() {
        assert_eq!(running_version().to_string(), env!("CARGO_PKG_VERSION"));
    }
}
