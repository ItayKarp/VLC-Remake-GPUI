//! The optional OpenSubtitles account (S6.5.1): downloads count against it, which allows more a
//! day than the address alone. Its username is in Settings → Privacy, its password in the OS
//! keychain; signing in gives a token, kept in the keychain too, after the username it is for,
//! so a changed username signs in again.
//!
//! Without a username or a saved password, downloads go without an account.

use super::api::Client;
use super::failure::Failure;
use crate::privacy::keychain::{self, Secret};

/// The token to download with for `username`: the kept one, or a new sign-in. `None` without
/// an account to sign in with.
pub(super) fn token(client: &Client, username: &str) -> Result<Option<String>, Failure> {
    let username = username.trim();
    if username.is_empty() {
        return Ok(None);
    }
    let Some(password) = keychain::load(Secret::OpenSubtitlesPassword).ok().flatten() else {
        return Ok(None);
    };
    if let Some(token) = kept(username) {
        return Ok(Some(token));
    }
    let token = client.login(username, &password)?;
    if let Err(error) = keychain::save(Secret::OpenSubtitlesToken, &stored(username, &token)) {
        tracing::warn!("opensubtitles: couldn't keep the token: {}", error.message());
    }
    Ok(Some(token))
}

/// Forgets the kept token, which OpenSubtitles no longer takes.
pub(super) fn forget_token() {
    if let Err(error) = keychain::forget(Secret::OpenSubtitlesToken) {
        tracing::warn!("opensubtitles: couldn't forget the token: {}", error.message());
    }
}

/// The kept token, when it is `username`'s.
fn kept(username: &str) -> Option<String> {
    let kept = keychain::load(Secret::OpenSubtitlesToken).ok().flatten()?;
    parse(&kept, username)
}

/// How a token is kept: the username, a new line, the token.
fn stored(username: &str, token: &str) -> String {
    format!("{username}\n{token}")
}

fn parse(kept: &str, username: &str) -> Option<String> {
    let (owner, token) = kept.split_once('\n')?;
    (owner == username && !token.is_empty()).then(|| token.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_is_only_its_owners() {
        let kept = stored("ana", "abc");
        assert_eq!(parse(&kept, "ana").as_deref(), Some("abc"));
        assert_eq!(parse(&kept, "bo"), None);
        assert_eq!(parse("garbage", "ana"), None);
    }
}
