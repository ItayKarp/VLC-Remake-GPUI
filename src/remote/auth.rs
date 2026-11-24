//! Signing in to the web remote: the password is checked against the one in the keychain, a
//! signed-in phone gets a random session token in a cookie, and an address that keeps guessing
//! wrong is shut out for a while.

use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::time::{Duration, Instant};

/// How many signed-in phones and browsers are remembered; signing in once more forgets the
/// oldest.
const MAX_SESSIONS: usize = 16;
/// Wrong passwords from one address before it is shut out.
const MAX_FAILURES: u32 = 5;
/// How long an address that guessed wrong too often is shut out.
pub const LOCKOUT: Duration = Duration::from_secs(60);
/// The digits a token is written in.
const HEX: &[u8; 16] = b"0123456789abcdef";
/// A session token's length in random bytes.
const TOKEN_BYTES: usize = 32;

/// A session token, as hex.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token(String);

impl Token {
    /// A new random token; `None` if the OS has no randomness to give.
    pub fn new() -> Option<Self> {
        let mut bytes = [0_u8; TOKEN_BYTES];
        getrandom::fill(&mut bytes).ok()?;
        let hex = bytes
            .iter()
            .flat_map(|byte| [HEX[usize::from(byte >> 4)], HEX[usize::from(byte & 15)]]);
        Some(Self(hex.map(char::from).collect()))
    }

    /// The token as the cookie carries it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The signed-in sessions, newest last.
#[derive(Debug, Default)]
pub struct Sessions(VecDeque<Token>);

impl Sessions {
    /// Remembers `token`, forgetting the oldest session past [`MAX_SESSIONS`].
    pub fn add(&mut self, token: Token) {
        if self.0.len() == MAX_SESSIONS {
            self.0.pop_front();
        }
        self.0.push_back(token);
    }

    /// Whether `token` is a signed-in session's.
    pub fn contains(&self, token: &str) -> bool {
        self.0.iter().any(|known| same_text(known.as_str(), token))
    }
}

/// Wrong passwords per address.
#[derive(Debug, Default)]
pub struct Lockout(HashMap<IpAddr, Failures>);

#[derive(Debug)]
struct Failures {
    count: u32,
    last: Instant,
}

impl Lockout {
    /// Whether `peer` is shut out at `now`.
    pub fn is_locked(&self, peer: IpAddr, now: Instant) -> bool {
        self.0.get(&peer).is_some_and(|failures| {
            failures.count >= MAX_FAILURES && now.duration_since(failures.last) < LOCKOUT
        })
    }

    /// Counts a wrong password from `peer` at `now`; a run of them restarts after a lockout's
    /// length without one.
    pub fn fail(&mut self, peer: IpAddr, now: Instant) {
        let failures = self.0.entry(peer).or_insert(Failures { count: 0, last: now });
        if now.duration_since(failures.last) >= LOCKOUT {
            failures.count = 0;
        }
        failures.count += 1;
        failures.last = now;
    }

    /// Forgets `peer`'s wrong passwords once it signed in.
    pub fn succeed(&mut self, peer: IpAddr) {
        self.0.remove(&peer);
    }
}

/// Whether `typed` is `saved`, taking as long whichever byte differs, so timing tells a
/// guesser nothing about how much of it was right.
pub fn same_text(saved: &str, typed: &str) -> bool {
    let (saved, typed) = (saved.as_bytes(), typed.as_bytes());
    saved.len() == typed.len()
        && saved.iter().zip(typed).fold(0, |diff, (a, b)| diff | (a ^ b)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_random_hex() {
        let (a, b) = (Token::new().unwrap(), Token::new().unwrap());
        assert_eq!(a.as_str().len(), TOKEN_BYTES * 2);
        assert!(a.as_str().bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn the_oldest_session_is_forgotten() {
        let mut sessions = Sessions::default();
        let first = Token::new().unwrap();
        sessions.add(first.clone());
        for _ in 0..MAX_SESSIONS {
            sessions.add(Token::new().unwrap());
        }
        assert!(!sessions.contains(first.as_str()));
        assert!(!sessions.contains(""));
    }

    #[test]
    fn guessing_shuts_an_address_out_for_a_while() {
        let (peer, other) = ("10.0.0.9".parse().unwrap(), "10.0.0.8".parse().unwrap());
        let start = Instant::now();
        let mut lockout = Lockout::default();
        for _ in 0..MAX_FAILURES {
            assert!(!lockout.is_locked(peer, start));
            lockout.fail(peer, start);
        }
        assert!(lockout.is_locked(peer, start));
        assert!(!lockout.is_locked(other, start));
        assert!(!lockout.is_locked(peer, start + LOCKOUT));
        lockout.fail(peer, start + LOCKOUT);
        assert!(!lockout.is_locked(peer, start + LOCKOUT));
    }

    #[test]
    fn passwords_compare_whole() {
        assert!(same_text("secret", "secret"));
        assert!(!same_text("secret", "secreT"));
        assert!(!same_text("secret", "secrets"));
    }
}
