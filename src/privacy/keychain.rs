//! Secrets in the OS keychain (S5.5.3): Keychain Services on macOS, Credential Manager on
//! Windows, the Secret Service on Linux. Secrets never go in a settings file; where there is no
//! keychain (a Linux session without a Secret Service), saving one is refused with why, never
//! written anywhere else.
//!
//! Every call may block (a keychain can ask the user, or talk to another process), so callers
//! run them off the UI thread.

use keyring::Entry;

/// The keychain service VLC's secrets are filed under.
const SERVICE: &str = "org.redesignedvlc.app";

/// A secret VLC keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Secret {
    /// The last.fm password (S5.5.2), for submitting played tracks (S8.17).
    LastFmPassword,
}

impl Secret {
    /// The account name it is filed under.
    fn account(self) -> &'static str {
        match self {
            Self::LastFmPassword => "lastfm",
        }
    }
}

/// Why the keychain didn't do what was asked, in words for the user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeychainError {
    /// There is no keychain to use.
    Unavailable(String),
    /// The keychain refused or failed.
    Failed(String),
}

impl KeychainError {
    /// The message the Settings row shows.
    pub fn message(&self) -> &str {
        match self {
            Self::Unavailable(message) | Self::Failed(message) => message,
        }
    }
}

/// What the keychain holds for a secret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// It is saved.
    Saved,
    /// It isn't.
    NotSaved,
    /// There is no keychain, or it can't be read: saving is off, and the message says why.
    Unavailable(String),
}

/// Whether `secret` is saved, without handing it out.
pub fn status(secret: Secret) -> Status {
    match entry(secret).and_then(|entry| read(&entry)) {
        Ok(Some(_)) => Status::Saved,
        Ok(None) => Status::NotSaved,
        Err(error) => Status::Unavailable(error.message().to_owned()),
    }
}

/// Saves `value` as `secret`, replacing what was saved.
pub fn save(secret: Secret, value: &str) -> Result<(), KeychainError> {
    entry(secret)?.set_password(value).map_err(|error| failed(&error))
}

/// Forgets `secret`; forgetting what isn't saved is fine.
pub fn forget(secret: Secret) -> Result<(), KeychainError> {
    match entry(secret)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(failed(&error)),
    }
}

/// The saved `secret`, if any, for the feature that sends it (S8.17).
#[allow(dead_code, reason = "scrobbling (S8.17) reads the password; S5.5 only saves it")]
pub fn load(secret: Secret) -> Result<Option<String>, KeychainError> {
    read(&entry(secret)?)
}

fn read(entry: &Entry) -> Result<Option<String>, KeychainError> {
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(failed(&error)),
    }
}

fn entry(secret: Secret) -> Result<Entry, KeychainError> {
    if let Err(error) = Entry::store_status() {
        tracing::warn!("keychain: none to use: {error}");
        return Err(KeychainError::Unavailable(UNAVAILABLE.to_owned()));
    }
    Entry::new(SERVICE, secret.account()).map_err(|error| failed(&error))
}

/// What the row says when there's no keychain.
const UNAVAILABLE: &str = if cfg!(target_os = "linux") {
    "No keychain: VLC needs a Secret Service, such as GNOME Keyring or KWallet, to save passwords."
} else {
    "The system keychain can't be reached, so the password can't be saved."
};

fn failed(error: &keyring::Error) -> KeychainError {
    tracing::warn!("keychain: {error}");
    match error {
        keyring::Error::NoStorageAccess(_) | keyring::Error::NoDefaultStore => {
            KeychainError::Unavailable(UNAVAILABLE.to_owned())
        }
        _ => KeychainError::Failed("The keychain couldn't be used. Try again.".to_owned()),
    }
}
