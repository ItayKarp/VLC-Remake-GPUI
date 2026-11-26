//! Submitting one batch of plays (S8.17.1): with the kept session, or signing in for one, and
//! signing in again once when last.fm says the session expired. What a failure means for the
//! queue is the [`Verdict`].
//!
//! The rules depend on a [`Service`], not on last.fm and the keychain directly: [`LastFm`] is the
//! one the app uses, and tests stand in their own.

use url::Url;

use super::Problem;
use super::api::{Client, Failure};
use super::session::{self, Account, NoSession};
use crate::db::scrobbles::Play;

/// What a submission needs from the settings.
#[derive(Clone)]
pub(super) struct Config {
    /// Who to scrobble as.
    pub(super) account: Account,
    /// The HTTP proxy in Settings → Network.
    pub(super) proxy: Option<Url>,
}

/// How submitting one batch went.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Verdict {
    /// last.fm took it.
    Sent,
    /// last.fm refused these plays for good: they are dropped.
    Refused,
    /// Nothing more can be sent now.
    Stopped(Problem),
}

/// What submitting needs: a session for the account, and a way to send plays with it.
trait Service {
    /// The session key for `account`: the kept one, or a new sign-in.
    fn session(&self, account: &Account) -> Result<String, NoSession>;
    /// Forgets the kept session, which the service no longer takes.
    fn forget_session(&self);
    /// Submits `plays` with the session `key`.
    fn scrobble(&self, key: &str, plays: &[Play]) -> Result<(), Failure>;
}

/// last.fm (or a service like it), with the session kept in the OS keychain.
struct LastFm {
    client: Client,
}

impl Service for LastFm {
    fn session(&self, account: &Account) -> Result<String, NoSession> {
        session::key(&self.client, account)
    }

    fn forget_session(&self) {
        session::forget();
    }

    fn scrobble(&self, key: &str, plays: &[Play]) -> Result<(), Failure> {
        self.client.scrobble(key, plays)
    }
}

/// Submits `plays` as `config` says. Blocks.
pub(super) fn submit(config: &Config, plays: &[Play]) -> Verdict {
    match Client::new(&config.account.url, config.proxy.as_ref()) {
        Ok(client) => submit_to(&LastFm { client }, &config.account, plays),
        Err(failure) => verdict(failure),
    }
}

/// Submits `plays` to `service` as `account`, signing in first when there is no session or it
/// expired.
fn submit_to(service: &dyn Service, account: &Account, plays: &[Play]) -> Verdict {
    let attempt = || -> Result<(), NoSession> {
        let key = service.session(account)?;
        match service.scrobble(&key, plays) {
            Err(Failure::SessionExpired) => {
                service.forget_session();
                let key = service.session(account)?;
                service.scrobble(&key, plays).map_err(NoSession::Failed)
            }
            other => other.map_err(NoSession::Failed),
        }
    };
    match attempt() {
        Ok(()) => Verdict::Sent,
        Err(NoSession::NoAccount) => Verdict::Stopped(Problem::NoAccount),
        Err(NoSession::Failed(failure)) => verdict(failure),
    }
}

/// What a failed submission means for the queue.
fn verdict(failure: Failure) -> Verdict {
    match failure {
        Failure::Refused(why) => {
            tracing::warn!("lastfm: plays refused, dropped: {why}");
            Verdict::Refused
        }
        Failure::NotSetUp => Verdict::Stopped(Problem::NotSetUp),
        Failure::LoginRejected | Failure::SessionExpired => {
            Verdict::Stopped(Problem::LoginRejected)
        }
        Failure::KeyRejected => Verdict::Stopped(Problem::KeyRejected),
        Failure::Offline(_) | Failure::Unavailable | Failure::Unreadable => {
            Verdict::Stopped(Problem::Offline)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;

    use super::*;

    /// A service answering from scripts, and noting what it was asked.
    #[derive(Default)]
    struct Script {
        sessions: RefCell<VecDeque<Result<String, NoSession>>>,
        scrobbles: RefCell<VecDeque<Result<(), Failure>>>,
        sent_with: RefCell<Vec<String>>,
        forgotten: RefCell<u32>,
    }

    impl Script {
        fn new(
            sessions: impl IntoIterator<Item = Result<String, NoSession>>,
            scrobbles: impl IntoIterator<Item = Result<(), Failure>>,
        ) -> Self {
            Self {
                sessions: RefCell::new(sessions.into_iter().collect()),
                scrobbles: RefCell::new(scrobbles.into_iter().collect()),
                ..Self::default()
            }
        }

        fn submit(&self) -> Verdict {
            let account = Account { username: "ana".into(), url: "https://x.test/".into() };
            let play = Play {
                artist: "A".into(),
                title: "T".into(),
                album: None,
                duration_s: None,
                started_s: 1,
            };
            submit_to(self, &account, &[play])
        }
    }

    impl Service for Script {
        fn session(&self, _: &Account) -> Result<String, NoSession> {
            self.sessions.borrow_mut().pop_front().expect("a session was scripted")
        }

        fn forget_session(&self) {
            *self.forgotten.borrow_mut() += 1;
        }

        fn scrobble(&self, key: &str, _: &[Play]) -> Result<(), Failure> {
            self.sent_with.borrow_mut().push(key.to_owned());
            self.scrobbles.borrow_mut().pop_front().expect("a scrobble was scripted")
        }
    }

    #[test]
    fn plays_go_with_the_session() {
        let script = Script::new([Ok("key".into())], [Ok(())]);
        assert_eq!(script.submit(), Verdict::Sent);
        assert_eq!(*script.sent_with.borrow(), ["key"]);
        assert_eq!(*script.forgotten.borrow(), 0);
    }

    #[test]
    fn an_expired_session_is_forgotten_and_signed_in_again_once() {
        let script = Script::new(
            [Ok("old".into()), Ok("new".into())],
            [Err(Failure::SessionExpired), Ok(())],
        );
        assert_eq!(script.submit(), Verdict::Sent);
        assert_eq!(*script.sent_with.borrow(), ["old", "new"]);
        assert_eq!(*script.forgotten.borrow(), 1);

        let expired = || Err(Failure::SessionExpired);
        let script = Script::new([Ok("old".into()), Ok("new".into())], [expired(), expired()]);
        assert_eq!(script.submit(), Verdict::Stopped(Problem::LoginRejected), "no third try");
    }

    #[test]
    fn without_an_account_nothing_is_sent() {
        let script = Script::new([Err(NoSession::NoAccount)], []);
        assert_eq!(script.submit(), Verdict::Stopped(Problem::NoAccount));
        assert!(script.sent_with.borrow().is_empty());
        let script = Script::new([Err(NoSession::Failed(Failure::LoginRejected))], []);
        assert_eq!(script.submit(), Verdict::Stopped(Problem::LoginRejected));
    }

    #[test]
    fn refused_plays_are_dropped_and_passing_failures_wait() {
        let script = Script::new([Ok("key".into())], [Err(Failure::Refused("bad".into()))]);
        assert_eq!(script.submit(), Verdict::Refused);
        for failure in [Failure::Offline("down".into()), Failure::Unavailable, Failure::Unreadable]
        {
            let script = Script::new([Ok("key".into())], [Err(failure)]);
            assert_eq!(script.submit(), Verdict::Stopped(Problem::Offline));
        }
    }

    #[test]
    fn the_apps_own_problems_stop_the_queue() {
        assert_eq!(verdict(Failure::NotSetUp), Verdict::Stopped(Problem::NotSetUp));
        assert_eq!(verdict(Failure::KeyRejected), Verdict::Stopped(Problem::KeyRejected));
    }
}
