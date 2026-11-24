//! Why OpenSubtitles didn't give what was asked (S6.5 edge cases), in words the Add Subtitles
//! dialog shows: offline, an outage, a rate limit, a used-up download quota with when it
//! resets, and a revoked API key.

use crate::subtitle_links::download;

/// Why a search or download failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// This build has no API key.
    NotSetUp,
    /// OpenSubtitles refused the app's API key: revoked or rotated.
    KeyRejected,
    /// A request needed a signed-in account (HTTP 401) and had none, or its token expired.
    Unauthorized,
    /// OpenSubtitles can't be reached; why, for the log.
    Offline(String),
    /// OpenSubtitles answered with a server error.
    Outage(u16),
    /// Too many requests (HTTP 429); the seconds to wait, when it said.
    RateLimited(Option<u64>),
    /// The day's downloads are used up.
    QuotaUsed(Quota),
    /// The username and password in Settings → Privacy were refused.
    LoginRejected,
    /// Another error status.
    Status(u16),
    /// The answer isn't what the API documents.
    Unreadable,
    /// The downloaded file couldn't be kept.
    File(download::Failure),
}

/// A used-up download quota: when it resets, and whether an account was signed in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quota {
    /// When more downloads are allowed, as OpenSubtitles words it ("5 hours and 2 minutes").
    pub resets_in: Option<String>,
    /// Whether the downloads counted against an account rather than the address.
    pub signed_in: bool,
}

impl Failure {
    /// What the dialog says about it.
    pub fn message(&self) -> String {
        match self {
            Self::NotSetUp => {
                tr!("This copy of VLC has no OpenSubtitles key, so it can't search there.")
                    .to_owned()
            }
            Self::KeyRejected => tr!("Update VLC to continue using OpenSubtitles.").to_owned(),
            Self::Unauthorized => tr!("OpenSubtitles wants an account to download: add yours in \
                     Settings → Privacy.")
            .to_owned(),
            Self::Offline(_) => {
                tr!("Couldn't reach OpenSubtitles. Check your internet connection.").to_owned()
            }
            Self::Outage(code) => {
                tr!("OpenSubtitles isn't answering right now (HTTP {code}). Try again later.", code)
            }
            Self::RateLimited(Some(seconds)) => {
                tr!("OpenSubtitles is busy. Try again in {seconds} s.", seconds)
            }
            Self::RateLimited(None) => {
                tr!("OpenSubtitles is busy. Try again in a moment.").to_owned()
            }
            Self::QuotaUsed(quota) => quota.message(),
            Self::LoginRejected => tr!("OpenSubtitles didn't accept the username and password in \
                     Settings → Privacy.")
            .to_owned(),
            Self::Status(code) => tr!("OpenSubtitles refused it (HTTP {code}).", code),
            Self::Unreadable => {
                tr!("OpenSubtitles answered with something VLC can't read.").to_owned()
            }
            Self::File(failure) => failure.message(),
        }
    }
}

impl Quota {
    fn message(&self) -> String {
        let when = self.resets_in.as_ref().map_or_else(
            || tr!("Try again tomorrow.").to_owned(),
            |resets_in| tr!("More are allowed in {resets_in}.", resets_in),
        );
        let mut sentences =
            vec![tr!("Today's OpenSubtitles downloads are used up.").to_owned(), when];
        if !self.signed_in {
            sentences.push(
                tr!("An OpenSubtitles account in Settings → Privacy allows more.").to_owned(),
            );
        }
        sentences.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_used_quota_says_when_it_resets() {
        let quota = Quota { resets_in: Some("5 hours".to_owned()), signed_in: true };
        assert_eq!(
            Failure::QuotaUsed(quota).message(),
            "Today's OpenSubtitles downloads are used up. More are allowed in 5 hours."
        );
        let anonymous = Quota { resets_in: None, signed_in: false };
        assert!(Failure::QuotaUsed(anonymous).message().ends_with("allows more."));
    }
}
