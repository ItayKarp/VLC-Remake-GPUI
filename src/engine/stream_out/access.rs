//! The access code a stream shared with the network asks for (S9.7.4): libVLC's HTTP and RTSP
//! outputs take a user name and password, so a stream anyone on the network could find is
//! watched only through its link, which carries the code. A stream for this computer alone has
//! none.

/// The user name viewers send; the code is the secret.
pub const VIEWER: &str = "vlc";
/// A code's length in characters.
const LENGTH: usize = 12;
/// The characters a code is made of: lowercase letters and digits, without the ones that look
/// alike (`0`/`o`, `1`/`l`/`i`), so a code read off a screen can be typed.
const ALPHABET: &[u8; 31] = b"abcdefghjkmnpqrstuvwxyz23456789";

/// A random access code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccessCode([u8; LENGTH]);

impl AccessCode {
    /// A new random code; `None` if the OS has no randomness to give.
    pub fn new() -> Option<Self> {
        let mut bytes = [0_u8; LENGTH];
        getrandom::fill(&mut bytes).ok()?;
        // 31 doesn't divide 256, so the first letters come up a little more often: about 70
        // bits of guessing either way, past what a stream's lifetime allows.
        Some(Self(bytes.map(|byte| ALPHABET[usize::from(byte) % ALPHABET.len()])))
    }

    /// The code as text.
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).expect("codes are made of ASCII letters and digits")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_random_and_easy_to_type() {
        let (a, b) = (AccessCode::new().unwrap(), AccessCode::new().unwrap());
        assert_ne!(a, b);
        assert_eq!(a.as_str().len(), LENGTH);
        assert!(a.as_str().bytes().all(|byte| ALPHABET.contains(&byte)));
    }
}
