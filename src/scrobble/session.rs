//! The last.fm session (S8.17.1): signing in with the username in Settings → Privacy and the
//! password in the OS keychain gives a session key, which doesn't expire. It is kept in the
//! keychain too, after the username and submission URL it is for, so changing either signs in
//! again. Every call here may block on the keychain or the network.

use super::api::{Client, Failure};
use crate::privacy::keychain::{self, Secret};

/// Who to scrobble as.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    /// The last.fm username.
    pub username: String,
    /// The submission URL.
    pub url: String,
}

/// Why there is no session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoSession {
    /// No username is set, or no password saved.
    NoAccount,
    /// Signing in failed.
    Failed(Failure),
}

/// The session key for `account`: the kept one, or a new sign-in with the saved password.
pub fn key(client: &Client, account: &Account) -> Result<String, NoSession> {
    let username = account.username.trim();
    if username.is_empty() {
        return Err(NoSession::NoAccount);
    }
    if let Some(key) = kept(account) {
        return Ok(key);
    }
    let password = keychain::load(Secret::LastFmPassword).ok().flatten();
    let password = password.filter(|password| !password.is_empty()).ok_or(NoSession::NoAccount)?;
    let key = client.sign_in(username, &password).map_err(NoSession::Failed)?;
    if let Err(error) = keychain::save(Secret::LastFmSession, &stored(account, &key)) {
        tracing::warn!("lastfm: couldn't keep the session: {}", error.message());
    }
    Ok(key)
}

/// Forgets the kept session, which the service no longer takes.
pub fn forget() {
    if let Err(error) = keychain::forget(Secret::LastFmSession) {
        tracing::warn!("lastfm: couldn't forget the session: {}", error.message());
    }
}

/// The kept session key, when it is `account`'s.
fn kept(account: &Account) -> Option<String> {
    parse(&keychain::load(Secret::LastFmSession).ok().flatten()?, account)
}

/// How a session is kept: the username, the URL and the key, a line each.
fn stored(account: &Account, key: &str) -> String {
    format!("{}\n{}\n{key}", account.username.trim(), account.url)
}

fn parse(kept: &str, account: &Account) -> Option<String> {
    let mut lines = kept.splitn(3, '\n');
    let (username, url, key) = (lines.next()?, lines.next()?, lines.next()?);
    let owned = username == account.username.trim() && url == account.url && !key.is_empty();
    owned.then(|| key.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(username: &str, url: &str) -> Account {
        Account { username: username.into(), url: url.into() }
    }

    #[test]
    fn a_session_is_only_its_accounts() {
        let ana = account("ana ", "https://ws.audioscrobbler.com/2.0/");
        let kept = stored(&ana, "abc");
        assert_eq!(parse(&kept, &ana).as_deref(), Some("abc"));
        assert_eq!(parse(&kept, &account("bo", &ana.url)), None);
        assert_eq!(parse(&kept, &account("ana", "https://libre.fm/2.0/")), None);
        assert_eq!(parse("garbage", &ana), None);
    }
}
