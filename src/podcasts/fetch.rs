//! Downloading a podcast feed (S8.17.2): a blocking GET, run off the UI thread, through the
//! HTTP proxy in Settings → Network when one is set, read as a [`Feed`].

use std::time::Duration;

use url::Url;

use super::feed::{self, Feed};

/// How long a download may take, connecting included.
const TIMEOUT: Duration = Duration::from_secs(30);
/// The most a feed may hold: long-running shows list thousands of episodes.
const MAX_BYTES: u64 = 16 * 1024 * 1024;
/// Names the app to the server.
const USER_AGENT: &str = concat!("RedesignedVLC/", env!("CARGO_PKG_VERSION"));

/// Why a feed couldn't be read, in words for the user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// What was typed isn't a web address.
    NotALink,
    /// The server answered with an error status.
    Status(u16),
    /// The feed is bigger than [`MAX_BYTES`].
    TooBig,
    /// What came back isn't a podcast feed.
    NotAFeed,
    /// The server couldn't be reached, or the connection failed; why, in words.
    Network(String),
}

impl Failure {
    /// What the app says about it.
    pub fn message(&self) -> String {
        match self {
            Self::NotALink => {
                tr!("Enter a feed address that starts with http:// or https://.").into()
            }
            Self::Status(404) => tr!("Nothing was found at that address.").into(),
            Self::Status(code) => tr!("The server refused the feed (HTTP {code}).", code),
            Self::TooBig => tr!("That feed is too big to read.").into(),
            Self::NotAFeed => tr!("That address isn't a podcast feed.").into(),
            Self::Network(why) => tr!("The feed couldn't be downloaded: {why}.", why),
        }
    }
}

/// The feed address `typed` means: a web link as it is, `feed://` and `podcast://` links and
/// bare host names as `https`.
pub fn address(typed: &str) -> Result<String, Failure> {
    let typed = typed.trim();
    let rest = typed.strip_prefix("feed://").or_else(|| typed.strip_prefix("podcast://"));
    let link = match rest {
        Some(rest) => format!("https://{rest}"),
        None if typed.contains("://") => typed.to_owned(),
        None => format!("https://{typed}"),
    };
    let parsed = Url::parse(&link).map_err(|_| Failure::NotALink)?;
    let web = matches!(parsed.scheme(), "http" | "https") && parsed.host_str().is_some();
    web.then(|| parsed.to_string()).ok_or(Failure::NotALink)
}

/// Downloads and reads the feed at `url`, through `proxy` (the environment's when `None`).
/// Blocks.
pub fn fetch(url: &str, proxy: Option<&Url>) -> Result<Feed, Failure> {
    let network = |error: ureq::Error| Failure::Network(error.to_string());
    let proxy = match proxy {
        Some(proxy) => Some(ureq::Proxy::new(proxy.as_str()).map_err(network)?),
        None => ureq::Proxy::try_from_env(),
    };
    let agent: ureq::Agent =
        ureq::Agent::config_builder().timeout_global(Some(TIMEOUT)).proxy(proxy).build().into();
    let answer = agent.get(url).header("User-Agent", USER_AGENT).call();
    let mut response = answer.map_err(|error| match error {
        ureq::Error::StatusCode(code) => Failure::Status(code),
        other => network(other),
    })?;
    let bytes = response.body_mut().with_config().limit(MAX_BYTES).read_to_vec();
    let bytes = bytes.map_err(|error| match error {
        ureq::Error::BodyExceedsLimit(_) => Failure::TooBig,
        other => network(other),
    })?;
    feed::parse(&String::from_utf8_lossy(&bytes)).map_err(|_| Failure::NotAFeed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_become_web_links() {
        let ok = |typed| address(typed).ok();
        assert_eq!(ok(" https://e.com/feed.xml "), Some("https://e.com/feed.xml".into()));
        assert_eq!(ok("feed://e.com/rss"), Some("https://e.com/rss".into()));
        assert_eq!(ok("podcast://e.com/rss"), Some("https://e.com/rss".into()));
        assert_eq!(ok("e.com/rss"), Some("https://e.com/rss".into()));
        assert_eq!(address("file:///etc/feed.xml"), Err(Failure::NotALink));
        assert_eq!(address("ftp://e.com/feed"), Err(Failure::NotALink));
        assert_eq!(address("   "), Err(Failure::NotALink));
    }
}
