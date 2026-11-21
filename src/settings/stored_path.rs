//! Lossless path storage.
//!
//! A path that is valid Unicode is stored as a plain TOML string, so the file stays
//! readable and hand-editable. A path that isn't (unpaired surrogates on Windows,
//! arbitrary bytes on Unix) is stored as `os-path:wide:<hex u16s>` or
//! `os-path:bytes:<hex bytes>`. Any path whose text starts with `os-path:` is also
//! encoded, so a plain string never collides with the encoded form.

use std::ffi::OsString;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Marks a value as an encoded path rather than plain text.
const PREFIX: &str = "os-path:";
/// Names the platform's native path encoding, so another OS family can reject it.
#[cfg(windows)]
const TAG: &str = "wide:";
/// Names the platform's native path encoding, so another OS family can reject it.
#[cfg(not(windows))]
const TAG: &str = "bytes:";

/// Encodes `path` for storage: as itself when it is plain Unicode, hex-encoded otherwise.
pub fn encode(path: &Path) -> String {
    match path.to_str() {
        Some(text) if !text.starts_with(PREFIX) => text.to_owned(),
        _ => format!("{PREFIX}{TAG}{}", hex_os(path.as_os_str())),
    }
}

/// Decodes a stored path. Returns `None` if the value is an encoded path that's
/// malformed or from another OS family.
pub fn decode(text: &str) -> Option<PathBuf> {
    let Some(rest) = text.strip_prefix(PREFIX) else {
        return Some(PathBuf::from(text));
    };
    let hex = rest.strip_prefix(TAG)?;
    unhex_os(hex).map(PathBuf::from)
}

#[cfg(windows)]
fn hex_os(s: &std::ffi::OsStr) -> String {
    use std::os::windows::ffi::OsStrExt;
    s.encode_wide().fold(String::new(), |mut out, unit| {
        // Writing to a `String` can't fail.
        let _ = write!(out, "{unit:04x}");
        out
    })
}

#[cfg(windows)]
fn unhex_os(hex: &str) -> Option<OsString> {
    use std::os::windows::ffi::OsStringExt;
    if !hex.is_ascii() || !hex.len().is_multiple_of(4) {
        return None;
    }
    let units = (0..hex.len())
        .step_by(4)
        .map(|i| u16::from_str_radix(&hex[i..i + 4], 16).ok())
        .collect::<Option<Vec<u16>>>()?;
    Some(OsString::from_wide(&units))
}

#[cfg(not(windows))]
fn hex_os(s: &std::ffi::OsStr) -> String {
    use std::os::unix::ffi::OsStrExt;
    s.as_bytes().iter().fold(String::new(), |mut out, b| {
        // Writing to a `String` can't fail.
        let _ = write!(out, "{b:02x}");
        out
    })
}

#[cfg(not(windows))]
fn unhex_os(hex: &str) -> Option<OsString> {
    use std::os::unix::ffi::OsStringExt;
    if !hex.is_ascii() || !hex.len().is_multiple_of(2) {
        return None;
    }
    let bytes = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect::<Option<Vec<u8>>>()?;
    Some(OsString::from_vec(bytes))
}

/// `#[serde(with = "stored_path::list")]` for `Vec<PathBuf>`. Reading is lenient: a
/// non-array value becomes an empty list and an unreadable entry is dropped, each with
/// a warning.
pub mod list {
    use serde::{Deserialize, Deserializer, Serializer};

    use super::{PathBuf, decode, encode};

    /// Writes the paths as a list of encoded strings.
    pub fn serialize<S: Serializer>(paths: &[PathBuf], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(paths.iter().map(|path| encode(path)))
    }

    /// Reads a list of encoded strings, dropping entries that can't be decoded.
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<PathBuf>, D::Error> {
        let value = toml::Value::deserialize(deserializer)?;
        let toml::Value::Array(items) = value else {
            tracing::warn!(
                "settings: expected a list of paths, found `{value}`; using an empty list"
            );
            return Ok(Vec::new());
        };
        Ok(items
            .into_iter()
            .filter_map(|item| {
                let path = item.as_str().and_then(decode);
                if path.is_none() {
                    tracing::warn!("settings: dropping unreadable path entry `{item}`");
                }
                path
            })
            .collect())
    }
}
