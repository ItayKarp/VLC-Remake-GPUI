//! The last.fm scrobbling API 2.0 (S8.17.1): signing in with `auth.getMobileSession` and
//! submitting plays with `track.scrobble`, each a blocking form POST run off the UI thread, to
//! the submission URL in Settings → Privacy (last.fm's, or a compatible service like Libre.fm).
//!
//! Every call is signed: an MD5 of its parameters, sorted by name, then the app's shared secret.
//! The API key and secret ship with the app, read when it is built from `RVLC_LASTFM_API_KEY`
//! and `RVLC_LASTFM_API_SECRET`; the same variables set when the app runs replace them. A build
//! without them says scrobbling isn't set up.

use std::fmt::Write as _;
use std::time::Duration;

use md5::{Digest, Md5};
use serde::Deserialize;
use url::Url;

use crate::db::scrobbles::Play;

/// The most plays one `track.scrobble` takes.
pub const MAX_BATCH: usize = 50;
/// How long a call may take.
const TIMEOUT: Duration = Duration::from_secs(30);
/// The most an answer may hold.
const MAX_ANSWER: u64 = 1024 * 1024;
/// Names the app to the service.
const USER_AGENT: &str = concat!("RedesignedVLC/", env!("CARGO_PKG_VERSION"));
/// The variables holding the API key and secret, when the app is built or, to replace them, run.
const KEY_VARIABLE: &str = "RVLC_LASTFM_API_KEY";
const SECRET_VARIABLE: &str = "RVLC_LASTFM_API_SECRET";

/// last.fm's error codes this client tells apart.
mod code {
    /// The user name or password is wrong.
    pub const AUTHENTICATION_FAILED: i64 = 4;
    /// A parameter is missing or wrong: the plays themselves are refused.
    pub const INVALID_PARAMETERS: i64 = 6;
    /// The session key no longer works.
    pub const INVALID_SESSION: i64 = 9;
    /// The API key isn't valid.
    pub const INVALID_API_KEY: i64 = 10;
    /// The service is down for a while.
    pub const OFFLINE: i64 = 11;
    /// The call's signature is wrong: the secret doesn't match the key.
    pub const INVALID_SIGNATURE: i64 = 13;
    /// Something failed for now.
    pub const TEMPORARY: i64 = 16;
    /// The API key was suspended.
    pub const SUSPENDED_KEY: i64 = 26;
    /// Too many calls.
    pub const RATE_LIMITED: i64 = 29;
}

/// Why a call didn't work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// This build has no API key or secret.
    NotSetUp,
    /// The service can't be reached (no network, a timeout), with the reason.
    Offline(String),
    /// The service is down or busy for now.
    Unavailable,
    /// The session key no longer works: sign in again.
    SessionExpired,
    /// The user name or password is wrong.
    LoginRejected,
    /// The app's key or secret is refused.
    KeyRejected,
    /// The plays were refused for good, with the service's words.
    Refused(String),
    /// The answer couldn't be read.
    Unreadable,
}

/// A client for one submission URL, holding the app's key and secret.
pub struct Client {
    agent: ureq::Agent,
    url: String,
    key: String,
    secret: String,
}

#[derive(Deserialize)]
struct Answer {
    error: Option<i64>,
    message: Option<String>,
    session: Option<Session>,
}

#[derive(Deserialize)]
struct Session {
    key: String,
}

impl Client {
    /// A client for `url`, going through `proxy` (the environment's when `None`).
    pub fn new(url: &str, proxy: Option<&Url>) -> Result<Self, Failure> {
        let (key, secret) = credentials().ok_or(Failure::NotSetUp)?;
        let proxy = match proxy {
            Some(proxy) => Some(ureq::Proxy::new(proxy.as_str()).map_err(|e| offline(&e))?),
            None => ureq::Proxy::try_from_env(),
        };
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .proxy(proxy)
            .http_status_as_error(false)
            .build()
            .into();
        Ok(Self { agent, url: url.to_owned(), key, secret })
    }

    /// Signs `username` in, answering the session key to scrobble with.
    pub fn sign_in(&self, username: &str, password: &str) -> Result<String, Failure> {
        let parameters = owned(&[
            ("method", "auth.getMobileSession"),
            ("username", username),
            ("password", password),
        ]);
        let answer = self.call(parameters).map_err(|failure| match failure {
            Failure::Refused(_) => Failure::LoginRejected,
            other => other,
        })?;
        answer.session.map(|session| session.key).ok_or(Failure::Unreadable)
    }

    /// Submits `plays` (the first [`MAX_BATCH`] of them) with `session`.
    pub fn scrobble(&self, session: &str, plays: &[Play]) -> Result<(), Failure> {
        let mut parameters = owned(&[("method", "track.scrobble"), ("sk", session)]);
        for (index, play) in plays.iter().take(MAX_BATCH).enumerate() {
            parameters.extend(play_parameters(index, play));
        }
        self.call(parameters).map(|_| ())
    }

    /// Makes the call `parameters` describe, signed.
    fn call(&self, mut parameters: Vec<(String, String)>) -> Result<Answer, Failure> {
        parameters.push(("api_key".to_owned(), self.key.clone()));
        let signature = signature(&parameters, &self.secret);
        parameters.push(("api_sig".to_owned(), signature));
        parameters.push(("format".to_owned(), "json".to_owned()));
        let answer = self
            .agent
            .post(&self.url)
            .header("User-Agent", USER_AGENT)
            .send_form(parameters.iter().map(|(name, value)| (name, value)));
        let mut response = answer.map_err(|error| offline(&error))?;
        let status = response.status().as_u16();
        let text = response.body_mut().with_config().limit(MAX_ANSWER).read_to_string();
        let text = text.map_err(|error| offline(&error))?;
        read(status, &text)
    }
}

/// `pairs` as owned parameters.
fn owned(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs.iter().map(|(name, value)| ((*name).to_owned(), (*value).to_owned())).collect()
}

/// The `[index]` parameters of one play.
fn play_parameters(index: usize, play: &Play) -> Vec<(String, String)> {
    let name = |field: &str| format!("{field}[{index}]");
    let mut parameters = vec![
        (name("artist"), play.artist.clone()),
        (name("track"), play.title.clone()),
        (name("timestamp"), play.started_s.to_string()),
    ];
    parameters.extend(play.album.clone().map(|album| (name("album"), album)));
    parameters.extend(play.duration_s.map(|seconds| (name("duration"), seconds.to_string())));
    parameters
}

/// The call's `api_sig`: the MD5 of every parameter's name and value, sorted by name, then the
/// secret, in lowercase hex. `format` and `callback` aren't signed.
fn signature(parameters: &[(String, String)], secret: &str) -> String {
    let mut sorted: Vec<&(String, String)> = parameters
        .iter()
        .filter(|(name, _)| !matches!(name.as_str(), "format" | "callback" | "api_sig"))
        .collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let mut hasher = Md5::new();
    for (name, value) in sorted {
        hasher.update(name.as_bytes());
        hasher.update(value.as_bytes());
    }
    hasher.update(secret.as_bytes());
    hasher.finalize().iter().fold(String::new(), |mut hex, byte| {
        // Writing to a `String` can't fail.
        let _ = write!(hex, "{byte:02x}");
        hex
    })
}

/// What an answer with HTTP `status` and body `text` means.
fn read(status: u16, text: &str) -> Result<Answer, Failure> {
    let Ok(answer) = serde_json::from_str::<Answer>(text) else {
        tracing::info!("lastfm: HTTP {status}, unreadable answer");
        return Err(if status >= 500 { Failure::Unavailable } else { Failure::Unreadable });
    };
    let Some(error) = answer.error else { return Ok(answer) };
    let message = answer.message.unwrap_or_default();
    tracing::info!("lastfm: error {error}: {message}");
    Err(failure(error, status, message))
}

/// What last.fm's error `code`, answered with HTTP `status`, means.
fn failure(error: i64, status: u16, message: String) -> Failure {
    match error {
        code::AUTHENTICATION_FAILED => Failure::LoginRejected,
        code::INVALID_SESSION => Failure::SessionExpired,
        code::INVALID_API_KEY | code::INVALID_SIGNATURE | code::SUSPENDED_KEY => {
            Failure::KeyRejected
        }
        code::OFFLINE | code::TEMPORARY | code::RATE_LIMITED => Failure::Unavailable,
        code::INVALID_PARAMETERS => Failure::Refused(message),
        _ if status >= 500 => Failure::Unavailable,
        _ => Failure::Refused(message),
    }
}

/// The API key and secret: the running app's variables, else the ones it was built with.
fn credentials() -> Option<(String, String)> {
    let pick = |variable: &str, built_with: Option<&str>| {
        let at_run_time = std::env::var(variable).ok();
        at_run_time.into_iter().chain(built_with.map(str::to_owned)).find(|v| !v.trim().is_empty())
    };
    let key = pick(KEY_VARIABLE, option_env!("RVLC_LASTFM_API_KEY"))?;
    let secret = pick(SECRET_VARIABLE, option_env!("RVLC_LASTFM_API_SECRET"))?;
    Some((key, secret))
}

fn offline(error: &ureq::Error) -> Failure {
    tracing::info!("lastfm: {error}");
    Failure::Offline(error.to_string())
}

#[cfg(test)]
mod tests;
