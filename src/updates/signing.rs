//! Trusting an update (S9.5.1): the manifest's Ed25519 signature, checked against the public
//! key the app was built with, and each package's SHA-256, which the signed manifest lists.
//!
//! Keys, signatures and hashes are written in lowercase hex. The key pair is made, and
//! manifests signed, by `cargo run --example update_manifest` (see `docs/adr/0005-updates.md`).

use std::fmt::Write as _;
use std::io::{self, Write};

use ring::digest::{Context, SHA256};
use ring::signature::{ED25519, UnparsedPublicKey};

/// The variable holding the public key, in hex: read when the app is built, and in debug builds
/// also when it runs (to try updates against a key of one's own).
pub const PUBLIC_KEY_VARIABLE: &str = "RVLC_UPDATE_PUBLIC_KEY";

/// Why a signature isn't trusted.
#[derive(Debug, PartialEq, Eq)]
pub enum Untrusted {
    /// The app was built without a public key, so nothing can be trusted.
    NoKey,
    /// The key or signature isn't hex.
    Malformed,
    /// The signature doesn't match the manifest and key.
    Forged,
}

/// The public key updates are signed for, as hex, if the app has one.
pub fn public_key() -> Option<String> {
    let at_run_time =
        cfg!(debug_assertions).then(|| std::env::var(PUBLIC_KEY_VARIABLE).ok()).flatten();
    let built_with = option_env!("RVLC_UPDATE_PUBLIC_KEY").map(str::to_owned);
    at_run_time.into_iter().chain(built_with).find(|key| !key.trim().is_empty())
}

/// Checks that `signature` (hex) is `key`'s (hex) signature of `message`.
pub fn verify(message: &[u8], signature: &str, key: Option<&str>) -> Result<(), Untrusted> {
    let key = from_hex(key.ok_or(Untrusted::NoKey)?).ok_or(Untrusted::Malformed)?;
    let signature = from_hex(signature).ok_or(Untrusted::Malformed)?;
    let key = UnparsedPublicKey::new(&ED25519, key);
    key.verify(message, &signature).map_err(|_| Untrusted::Forged)
}

/// Writes through to `inner` while hashing what passes, so a download is hashed as it is
/// saved.
pub struct HashingWriter<W> {
    inner: W,
    hash: Context,
}

impl<W: Write> HashingWriter<W> {
    /// Hashes what is written to `inner`.
    pub fn new(inner: W) -> Self {
        Self { inner, hash: Context::new(&SHA256) }
    }

    /// The SHA-256 of everything written, in hex, and the writer.
    pub fn finish(self) -> (String, W) {
        (to_hex(self.hash.finish().as_ref()), self.inner)
    }
}

impl<W: Write> Write for HashingWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(bytes)?;
        self.hash.update(&bytes[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// `bytes` in lowercase hex.
pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::with_capacity(bytes.len() * 2), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

/// The bytes `text` writes in hex (either case, surrounding white space ignored); `None` when
/// it isn't hex.
pub fn from_hex(text: &str) -> Option<Vec<u8>> {
    let text = text.trim();
    if !text.len().is_multiple_of(2) || !text.is_ascii() {
        return None;
    }
    (0..text.len()).step_by(2).map(|at| u8::from_str_radix(&text[at..at + 2], 16).ok()).collect()
}

#[cfg(test)]
mod tests {
    use ring::rand::SystemRandom;
    use ring::signature::{Ed25519KeyPair, KeyPair};

    use super::*;

    fn key_pair() -> Ed25519KeyPair {
        let document = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).unwrap();
        Ed25519KeyPair::from_pkcs8(document.as_ref()).unwrap()
    }

    #[test]
    fn a_signature_is_trusted_only_for_its_message_and_key() {
        let pair = key_pair();
        let key = to_hex(pair.public_key().as_ref());
        let signature = to_hex(pair.sign(b"manifest").as_ref());
        assert_eq!(verify(b"manifest", &signature, Some(&key)), Ok(()));
        assert_eq!(verify(b"manifesto", &signature, Some(&key)), Err(Untrusted::Forged));
        let other = to_hex(key_pair().public_key().as_ref());
        assert_eq!(verify(b"manifest", &signature, Some(&other)), Err(Untrusted::Forged));
        assert_eq!(verify(b"manifest", &signature, None), Err(Untrusted::NoKey));
        assert_eq!(verify(b"manifest", "zz", Some(&key)), Err(Untrusted::Malformed));
    }

    #[test]
    fn hashes_what_is_written() {
        let mut writer = HashingWriter::new(Vec::new());
        writer.write_all(b"abc").unwrap();
        let (hash, bytes) = writer.finish();
        assert_eq!(hash, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(bytes, b"abc");
    }

    #[test]
    fn hex_round_trips() {
        assert_eq!(from_hex(" 00ff7A\n"), Some(vec![0, 255, 122]));
        assert_eq!(to_hex(&[0, 255, 122]), "00ff7a");
        assert_eq!(from_hex("abc"), None);
        assert_eq!(from_hex("zz"), None);
        assert_eq!(from_hex("é1"), None);
    }
}
