//! The OpenSubtitles REST API (S6.5.1, `api.opensubtitles.com/api/v1`): searching, asking for a
//! file's download link, and signing in, each a blocking request run off the UI thread.
//!
//! Every request carries the app's API key and a `User-Agent` naming the app, as the API
//! requires. The key is shipped with the app: it is read when the app is built, from
//! `RVLC_OPENSUBTITLES_API_KEY`, and the same variable set when the app runs replaces it (to try
//! another key). A build without one says OpenSubtitles isn't set up.

use serde::Deserialize;
use serde::de::DeserializeOwned;
use ureq::http::Response;
use url::Url;

use super::failure::{Failure, Quota};
use super::results::{Page, Parameters};
use crate::subtitle_links::download;

/// Where the API is.
const BASE: &str = "https://api.opensubtitles.com/api/v1";
/// Names the app to the API, which refuses requests without one.
const USER_AGENT: &str = concat!("RedesignedVLC v", env!("CARGO_PKG_VERSION"));
/// The variable holding the API key, when the app is built or, to replace it, run.
const KEY_VARIABLE: &str = "RVLC_OPENSUBTITLES_API_KEY";
/// The most an API answer may hold.
const MAX_ANSWER: u64 = 4 * 1024 * 1024;
/// HTTP statuses the API answers with.
const UNAUTHORIZED: u16 = 401;
const FORBIDDEN: u16 = 403;
const QUOTA_USED: u16 = 406;
const TOO_MANY: u16 = 429;
const SERVER_ERRORS: u16 = 500;

/// A client for the API, holding the key and the connection settings.
pub(super) struct Client {
    agent: ureq::Agent,
    key: String,
}

/// What `POST /download` answers.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct Link {
    /// Where the file can be fetched, for a few hours.
    #[serde(rename = "link")]
    pub(super) url: String,
    /// The file's name.
    pub(super) file_name: Option<String>,
    /// The downloads left today.
    remaining: Option<i64>,
    /// When the quota resets, in words.
    reset_time: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Login {
    token: String,
}

impl Client {
    /// A client going through `proxy` (the environment's when `None`); fails when this build
    /// has no API key.
    pub(super) fn new(proxy: Option<&Url>) -> Result<Self, Failure> {
        let key = api_key().ok_or(Failure::NotSetUp)?;
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(download::TIMEOUT))
            .proxy(download::proxy_for(proxy).map_err(Failure::File)?)
            .http_status_as_error(false)
            .build()
            .into();
        Ok(Self { agent, key })
    }

    /// One page of subtitles matching `parameters`.
    pub(super) fn search(&self, parameters: &Parameters) -> Result<Page, Failure> {
        let mut request = self.agent.get(format!("{BASE}/subtitles"));
        for (name, value) in parameters {
            request = request.query(*name, value);
        }
        let answer = request
            .header("Api-Key", &self.key)
            .header("User-Agent", USER_AGENT)
            .header("Accept", "application/json");
        read(answer.call(), false)
    }

    /// The download link of file `file_id`, counted against `token`'s account when signed in.
    pub(super) fn link(&self, file_id: &str, token: Option<&str>) -> Result<Link, Failure> {
        let id: u64 = file_id.parse().map_err(|_| Failure::Unreadable)?;
        let mut request = self.post("download");
        if let Some(token) = token {
            request = request.header("Authorization", &format!("Bearer {token}"));
        }
        let body = json(&serde_json::json!({ "file_id": id }));
        let link: Link =
            read(request.send(body.as_str()), true).map_err(|failure| match failure {
                Failure::QuotaUsed(quota) => {
                    Failure::QuotaUsed(Quota { signed_in: token.is_some(), ..quota })
                }
                other => other,
            })?;
        if link.url.is_empty() {
            let used = link.remaining.is_some_and(|remaining| remaining <= 0);
            let quota = Quota { resets_in: link.reset_time, signed_in: token.is_some() };
            return Err(if used { Failure::QuotaUsed(quota) } else { Failure::Unreadable });
        }
        Ok(link)
    }

    /// Signs `username` in, answering the token that counts downloads against the account.
    pub(super) fn login(&self, username: &str, password: &str) -> Result<String, Failure> {
        let body = json(&serde_json::json!({ "username": username, "password": password }));
        let login: Login =
            read(self.post("login").send(body.as_str()), false).map_err(|failure| {
                if failure == Failure::Unauthorized { Failure::LoginRejected } else { failure }
            })?;
        Ok(login.token)
    }

    /// The file a download link names, at most the download limit of it.
    pub(super) fn fetch(&self, link: &str) -> Result<Vec<u8>, Failure> {
        let mut response = self.agent.get(link).call().map_err(|error| offline(&error))?;
        let status = response.status().as_u16();
        if !response.status().is_success() {
            return Err(failure_for(status, retry_after(&response), None));
        }
        download::read_body(response.body_mut()).map_err(Failure::File)
    }

    fn post(&self, path: &str) -> ureq::RequestBuilder<ureq::typestate::WithBody> {
        self.agent
            .post(format!("{BASE}/{path}"))
            .header("Api-Key", &self.key)
            .header("User-Agent", USER_AGENT)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
    }
}

/// The API key: the running app's variable, else the one the app was built with.
fn api_key() -> Option<String> {
    let at_run_time = std::env::var(KEY_VARIABLE).ok();
    let built_with = option_env!("RVLC_OPENSUBTITLES_API_KEY").map(str::to_owned);
    at_run_time.into_iter().chain(built_with).find(|key| !key.trim().is_empty())
}

fn json(value: &serde_json::Value) -> String {
    value.to_string()
}

/// An answer read as `T`, or the failure its status means. `quota_counts` when a 406 means the
/// download quota (whose body says when it resets).
fn read<T: DeserializeOwned>(
    answer: Result<Response<ureq::Body>, ureq::Error>,
    quota_counts: bool,
) -> Result<T, Failure> {
    let mut response = answer.map_err(|error| offline(&error))?;
    let status = response.status().as_u16();
    let wait = retry_after(&response);
    let text = response.body_mut().with_config().limit(MAX_ANSWER).read_to_string();
    if !response.status().is_success() {
        let body = text.ok().filter(|_| quota_counts && status == QUOTA_USED);
        return Err(failure_for(status, wait, body.as_deref()));
    }
    let text = text.map_err(|error| offline(&error))?;
    serde_json::from_str(&text).map_err(|error| {
        tracing::warn!("opensubtitles: unreadable answer: {error}");
        Failure::Unreadable
    })
}

/// What an error `status` means; `quota` is a 406's body, saying when the quota resets.
fn failure_for(status: u16, wait: Option<u64>, quota: Option<&str>) -> Failure {
    tracing::info!("opensubtitles: HTTP {status}");
    match status {
        UNAUTHORIZED => Failure::Unauthorized,
        FORBIDDEN => Failure::KeyRejected,
        TOO_MANY => Failure::RateLimited(wait),
        QUOTA_USED if quota.is_some() => {
            let link: Link =
                quota.and_then(|body| serde_json::from_str(body).ok()).unwrap_or_default();
            Failure::QuotaUsed(Quota { resets_in: link.reset_time, signed_in: false })
        }
        code if code >= SERVER_ERRORS => Failure::Outage(code),
        code => Failure::Status(code),
    }
}

/// The seconds a 429 asks to wait, from `Retry-After` or `X-RateLimit-Reset`.
fn retry_after(response: &Response<ureq::Body>) -> Option<u64> {
    ["retry-after", "ratelimit-reset", "x-ratelimit-reset"]
        .iter()
        .find_map(|name| response.headers().get(*name)?.to_str().ok()?.trim().parse::<u64>().ok())
}

fn offline(error: &ureq::Error) -> Failure {
    tracing::info!("opensubtitles: {error}");
    Failure::Offline(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_say_what_went_wrong() {
        assert_eq!(failure_for(403, None, None), Failure::KeyRejected);
        assert_eq!(failure_for(429, Some(3), None), Failure::RateLimited(Some(3)));
        assert_eq!(failure_for(503, None, None), Failure::Outage(503));
        assert_eq!(failure_for(406, None, None), Failure::Status(406));
        let body = r#"{"remaining":-1,"reset_time":"7 hours and 2 minutes"}"#;
        let Failure::QuotaUsed(quota) = failure_for(406, None, Some(body)) else { panic!() };
        assert_eq!(quota.resets_in.as_deref(), Some("7 hours and 2 minutes"));
    }
}
