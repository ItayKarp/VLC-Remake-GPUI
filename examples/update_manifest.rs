//! Makes the update key pair, and writes and signs a release's update manifest (S9.5.1).
//!
//! - `cargo run --example update_manifest -- keygen <private key file>` makes an Ed25519 key
//!   pair: the private key goes to the file (keep it secret, out of the repository), the public
//!   key is printed, for building the app with `RVLC_UPDATE_PUBLIC_KEY`.
//! - `cargo run --example update_manifest -- sign [folder]` writes `update-manifest.json` for
//!   the packages in the folder (default `dist`) and signs it into `update-manifest.json.sig`,
//!   with the private key from `RVLC_UPDATE_SIGNING_KEY` (hex) or the file
//!   `RVLC_UPDATE_SIGNING_KEY_FILE` names. The version is Cargo.toml's (`RVLC_RELEASE_VERSION`
//!   names another, to try updates out); the links point at the GitHub release `v<version>` of
//!   `GITHUB_REPO` (default `ItayKarp/VLC-Remake-GPUI`), or below `RVLC_RELEASE_URL`.
//!
//! Packages are recognized by the names `packaging/` gives them: `…-macos-<arch>.app.zip`,
//! `…-windows-x64.msi` and `…-<arch>.AppImage`. The format is in `docs/adr/0005-updates.md`.

use std::error::Error;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io;
use std::path::Path;

use ring::digest::{Context, SHA256};
use ring::rand::SystemRandom;
use ring::signature::{Ed25519KeyPair, KeyPair};
use serde_json::json;

/// Any failure ends the run with its message.
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

/// The manifest's file name, which the app asks the latest release for.
const MANIFEST: &str = "update-manifest.json";
/// The repository releases are published to, unless `GITHUB_REPO` says otherwise.
const DEFAULT_REPO: &str = "ItayKarp/VLC-Remake-GPUI";

fn main() -> Result {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match arguments.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["keygen", file] => keygen(Path::new(file)),
        ["sign"] => sign(Path::new("dist")),
        ["sign", folder] => sign(Path::new(folder)),
        _ => Err("usage: update_manifest keygen <private key file> | sign [folder]".into()),
    }
}

/// Makes a key pair: the private key into `file`, the public key printed.
fn keygen(file: &Path) -> Result {
    if file.exists() {
        return Err(format!("{} exists; not replacing a key", file.display()).into());
    }
    let document = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new())
        .map_err(|_| "the system has no randomness to make a key with")?;
    fs::write(file, hex(document.as_ref()))?;
    restrict(file)?;
    let pair = Ed25519KeyPair::from_pkcs8(document.as_ref()).map_err(|error| error.to_string())?;
    println!("Private key written to {}.", file.display());
    println!("RVLC_UPDATE_PUBLIC_KEY={}", hex(pair.public_key().as_ref()));
    Ok(())
}

/// Writes and signs the manifest for the packages in `folder`.
fn sign(folder: &Path) -> Result {
    let pair = signing_key()?;
    let version = std::env::var("RVLC_RELEASE_VERSION")
        .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_owned());
    let version = version.as_str();
    let repo = std::env::var("GITHUB_REPO").unwrap_or_else(|_| DEFAULT_REPO.to_owned());
    let release = format!("https://github.com/{repo}/releases");
    let release = std::env::var("RVLC_RELEASE_URL").unwrap_or(release);
    let base = format!("{release}/download/v{version}");
    let mut packages = Vec::new();
    for entry in fs::read_dir(folder)? {
        let path = entry?.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        if let Some((os, arch, kind)) = recognize(&name, version) {
            let (size, sha256) = measure(&path)?;
            let url = format!("{base}/{name}");
            packages.push(json!({ "os": os, "arch": arch, "kind": kind, "url": url,
                                  "size": size, "sha256": sha256 }));
            println!("{os} {arch} {kind}: {name}");
        }
    }
    let manifest = json!({
        "version": version,
        "notes_url": format!("{release}/tag/v{version}"),
        "packages": packages,
    });
    let text = serde_json::to_string_pretty(&manifest)? + "\n";
    fs::write(folder.join(MANIFEST), &text)?;
    let signature = hex(pair.sign(text.as_bytes()).as_ref());
    fs::write(folder.join(format!("{MANIFEST}.sig")), signature + "\n")?;
    println!("Signed {} for {version}.", folder.join(MANIFEST).display());
    Ok(())
}

/// The private key from `RVLC_UPDATE_SIGNING_KEY`, or the file `RVLC_UPDATE_SIGNING_KEY_FILE`
/// names.
fn signing_key() -> Result<Ed25519KeyPair> {
    let text = match std::env::var("RVLC_UPDATE_SIGNING_KEY") {
        Ok(text) if !text.trim().is_empty() => text,
        _ => match std::env::var("RVLC_UPDATE_SIGNING_KEY_FILE") {
            Ok(file) => fs::read_to_string(file)?,
            Err(_) => {
                return Err("set RVLC_UPDATE_SIGNING_KEY or RVLC_UPDATE_SIGNING_KEY_FILE".into());
            }
        },
    };
    let document = unhex(text.trim()).ok_or("the signing key isn't hex")?;
    Ok(Ed25519KeyPair::from_pkcs8(&document).map_err(|error| error.to_string())?)
}

/// The OS, architecture (Rust's names) and kind of the package named `name`, if it is one.
fn recognize(name: &str, version: &str) -> Option<(&'static str, String, &'static str)> {
    let rest = name.strip_prefix(&format!("RedesignedVLC-{version}-"))?;
    if let Some(arch) = rest.strip_prefix("macos-").and_then(|r| r.strip_suffix(".app.zip")) {
        return Some(("macos", rust_arch(arch), "app"));
    }
    if let Some(arch) = rest.strip_prefix("windows-").and_then(|r| r.strip_suffix(".msi")) {
        return Some(("windows", rust_arch(arch), "msi"));
    }
    let arch = rest.strip_suffix(".AppImage")?;
    Some(("linux", rust_arch(arch), "appimage"))
}

/// `arch` as Rust's `std::env::consts::ARCH` names it.
fn rust_arch(arch: &str) -> String {
    match arch {
        "arm64" => "aarch64",
        "x64" | "amd64" => "x86_64",
        other => other,
    }
    .to_owned()
}

/// The size and SHA-256 (hex) of the file at `path`.
fn measure(path: &Path) -> Result<(u64, String)> {
    let mut hash = Hasher(Context::new(&SHA256));
    let size = io::copy(&mut File::open(path)?, &mut hash)?;
    Ok((size, hex(hash.0.finish().as_ref())))
}

/// Feeds what is written to a SHA-256.
struct Hasher(Context);

impl io::Write for Hasher {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::with_capacity(bytes.len() * 2), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.is_ascii() {
        return None;
    }
    (0..text.len()).step_by(2).map(|at| u8::from_str_radix(&text[at..at + 2], 16).ok()).collect()
}

/// Lets only the owner read the private key.
#[cfg(unix)]
fn restrict(file: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(file, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn restrict(_: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_the_packages_packaging_makes() {
        let found = |name| recognize(name, "1.2.0");
        let mac = found("RedesignedVLC-1.2.0-macos-arm64.app.zip");
        assert_eq!(mac, Some(("macos", "aarch64".into(), "app")));
        let universal = found("RedesignedVLC-1.2.0-macos-universal.app.zip");
        assert_eq!(universal, Some(("macos", "universal".into(), "app")));
        let msi = found("RedesignedVLC-1.2.0-windows-x64.msi");
        assert_eq!(msi, Some(("windows", "x86_64".into(), "msi")));
        let appimage = found("RedesignedVLC-1.2.0-aarch64.AppImage");
        assert_eq!(appimage, Some(("linux", "aarch64".into(), "appimage")));
        assert_eq!(found("RedesignedVLC-1.2.0-macos-arm64.dmg"), None);
        assert_eq!(found("RedesignedVLC-1.1.0-windows-x64.msi"), None);
        assert_eq!(found("redesigned-vlc_1.2.0_arm64.deb"), None);
    }

    #[test]
    fn signs_what_the_app_verifies() {
        let document = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).unwrap();
        let text = hex(document.as_ref());
        let pair = Ed25519KeyPair::from_pkcs8(&unhex(&text).unwrap()).unwrap();
        let signature = pair.sign(b"manifest");
        let key = ring::signature::UnparsedPublicKey::new(
            &ring::signature::ED25519,
            pair.public_key().as_ref(),
        );
        assert!(key.verify(b"manifest", signature.as_ref()).is_ok());
    }
}
