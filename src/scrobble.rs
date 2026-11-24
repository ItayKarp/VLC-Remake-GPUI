//! Scrobbling to last.fm (S8.17.1): with Settings → Privacy's "Submit played tracks to last.fm"
//! on, each track that counts as played ([`listening`]) is queued in the database
//! ([`crate::db::scrobbles`]) and submitted in batches ([`api`]) as the account in Settings →
//! Privacy ([`session`]). A track needs an artist and a title tag to be scrobbled.
//!
//! The [`Scrobbler`] global runs one submission at a time. When last.fm can't be reached the
//! plays stay queued (capped) and are sent with the next play, or after [`RETRY`]; when the
//! account or the app's key is refused they stay queued until the account changes. What went
//! wrong is shown once ([`take_problem`]), not again until a submission works or something else
//! goes wrong.

pub mod api;
pub mod listening;
mod session;

use std::time::Duration;

use gpui::{App, AsyncApp, Global};
use url::Url;

use self::api::{Client, Failure, MAX_BATCH};
use self::session::{Account, NoSession};
use crate::db::scrobbles::{self, Play, Queued};
use crate::history::History;
use crate::settings::{self, SettingsStore};

/// How long after a passing failure the queue is tried again.
pub const RETRY: Duration = Duration::from_mins(5);

/// What stops plays from being submitted, as the user is told.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    /// This build has no API key.
    NotSetUp,
    /// No username, or no password saved.
    NoAccount,
    /// last.fm refused the username or password.
    LoginRejected,
    /// last.fm refused the app's key.
    KeyRejected,
    /// last.fm can't be reached for now.
    Offline,
}

impl Problem {
    /// What the main window's toast says.
    pub fn message(self) -> &'static str {
        match self {
            Self::NotSetUp => tr!("This build of VLC can't submit to last.fm: it has no API key."),
            Self::NoAccount => tr!(
                "To submit played tracks to last.fm, add your username and password in Settings \
                 → Privacy."
            ),
            Self::LoginRejected => tr!(
                "last.fm didn't accept your username or password. Played tracks are kept until \
                 you fix them in Settings → Privacy."
            ),
            Self::KeyRejected => {
                tr!("last.fm refused VLC's API key. Played tracks are kept for now.")
            }
            Self::Offline => {
                tr!("last.fm can't be reached. Played tracks are kept and sent later.")
            }
        }
    }

    /// Whether to try again by itself, rather than wait for the account to change.
    fn retries(self) -> bool {
        self == Self::Offline
    }
}

/// The submission state.
#[derive(Default)]
pub struct Scrobbler {
    /// Whether a submission is running.
    sending: bool,
    /// Why the last submission stopped, until a later one works.
    problem: Option<Problem>,
    /// Whether that problem was shown.
    shown: bool,
    /// The account settings at the last look, to notice a change.
    account: Option<(String, String)>,
}

impl Global for Scrobbler {}

/// Starts submitting: what was queued last time goes now, and changing the account tries again.
pub fn init(cx: &mut App) {
    cx.set_global(Scrobbler { account: account_settings(cx), ..Scrobbler::default() });
    cx.observe_global::<SettingsStore>(|cx| {
        let now = account_settings(cx);
        let changed = cx.global::<Scrobbler>().account != now;
        cx.global_mut::<Scrobbler>().account = now;
        if changed {
            unblock(cx);
        }
    })
    .detach();
    flush(cx);
}

/// Queues `play` and submits what is queued, if scrobbling is on.
pub fn scrobble(play: Play, cx: &mut App) {
    if !settings::get(cx).prefs.privacy.lastfm.submit {
        return;
    }
    let Some(history) = History::get(cx) else { return };
    history.with_database(move |connection| {
        if let Err(error) = scrobbles::enqueue(connection, &play) {
            tracing::warn!("lastfm: couldn't queue a play: {error}");
        }
    });
    flush(cx);
}

/// The last.fm password was saved or forgotten: the kept session goes, and the queue is tried
/// again with the new one.
pub fn password_changed(cx: &mut App) {
    cx.background_executor().spawn(async { session::forget() }).detach();
    unblock(cx);
}

/// Why submitting stopped, the first time it is asked after it happened; the main window shows
/// it.
pub fn take_problem(cx: &mut App) -> Option<Problem> {
    let scrobbler = cx.try_global::<Scrobbler>()?;
    if scrobbler.shown {
        return None;
    }
    let problem = scrobbler.problem?;
    cx.global_mut::<Scrobbler>().shown = true;
    Some(problem)
}

/// Forgets why submitting stopped and tries again.
fn unblock(cx: &mut App) {
    if let Some(scrobbler) = cx.try_global::<Scrobbler>()
        && scrobbler.problem.is_some()
    {
        cx.global_mut::<Scrobbler>().problem = None;
        cx.global_mut::<Scrobbler>().shown = false;
    }
    flush(cx);
}

/// What a submission needs from the settings.
#[derive(Clone)]
struct Config {
    account: Account,
    proxy: Option<Url>,
}

/// The username and submission URL, while scrobbling is on.
fn account_settings(cx: &App) -> Option<(String, String)> {
    let lastfm = &settings::get(cx).prefs.privacy.lastfm;
    lastfm.submit.then(|| (lastfm.username.trim().to_owned(), lastfm.submission_url.clone()))
}

/// Submits what is queued, unless a submission runs, scrobbling is off, or a problem that
/// retrying won't fix stopped the last one.
fn flush(cx: &mut App) {
    let Some((username, url)) = account_settings(cx) else { return };
    let proxy = settings::proxy_url(&settings::get(cx).prefs.network.http_proxy);
    let Some(scrobbler) = cx.try_global::<Scrobbler>() else { return };
    if scrobbler.sending || scrobbler.problem.is_some_and(|problem| !problem.retries()) {
        return;
    }
    cx.global_mut::<Scrobbler>().sending = true;
    let config = Config { account: Account { username, url }, proxy };
    cx.spawn(async move |cx| {
        let outcome = send_all(&config, cx).await;
        cx.update(|cx| finished(outcome, cx));
    })
    .detach();
}

/// Notes how a submission ended; after a passing failure, tries again in a while.
fn finished(outcome: Result<(), Problem>, cx: &mut App) {
    let scrobbler = cx.global_mut::<Scrobbler>();
    scrobbler.sending = false;
    let Err(problem) = outcome else {
        scrobbler.problem = None;
        scrobbler.shown = false;
        return;
    };
    if scrobbler.problem != Some(problem) {
        scrobbler.problem = Some(problem);
        scrobbler.shown = false;
    }
    if problem.retries() {
        cx.spawn(async |cx| {
            cx.background_executor().timer(RETRY).await;
            cx.update(flush);
        })
        .detach();
    }
}

/// Submits the queue a batch at a time until it is empty or a submission fails.
async fn send_all(config: &Config, cx: &mut AsyncApp) -> Result<(), Problem> {
    loop {
        let batch = next_batch(cx).await;
        if batch.is_empty() {
            return Ok(());
        }
        let ids: Vec<i64> = batch.iter().map(|queued| queued.id).collect();
        let plays: Vec<Play> = batch.into_iter().map(|queued| queued.play).collect();
        let sending = config.clone();
        let verdict = cx.background_executor().spawn(async move { submit(&sending, &plays) });
        match verdict.await {
            Verdict::Sent | Verdict::Refused => remove(ids, cx),
            Verdict::Stopped(problem) => return Err(problem),
        }
    }
}

/// The oldest plays waiting, at most a batch of them.
async fn next_batch(cx: &mut AsyncApp) -> Vec<Queued> {
    let now_s = crate::db::now_ms() / 1000;
    let answer = cx.update(|cx| {
        History::get(cx).map(|history| {
            history.query("scrobbles", move |connection| {
                scrobbles::next_batch(connection, now_s, MAX_BATCH)
            })
        })
    });
    match answer {
        Some(answer) => answer.await.ok().flatten().unwrap_or_default(),
        None => Vec::new(),
    }
}

/// Takes the plays `ids` off the queue.
fn remove(ids: Vec<i64>, cx: &mut AsyncApp) {
    cx.update(|cx| {
        let Some(history) = History::get(cx) else { return };
        history.with_database(move |connection| {
            if let Err(error) = scrobbles::remove(connection, &ids) {
                tracing::warn!("lastfm: couldn't take plays off the queue: {error}");
            }
        });
    });
}

/// How submitting one batch went.
enum Verdict {
    /// last.fm took it.
    Sent,
    /// last.fm refused these plays for good: they are dropped.
    Refused,
    /// Nothing more can be sent now.
    Stopped(Problem),
}

/// Submits `plays`, signing in first when there is no session or it expired. Blocks.
fn submit(config: &Config, plays: &[Play]) -> Verdict {
    let attempt = || -> Result<(), NoSession> {
        let client =
            Client::new(&config.account.url, config.proxy.as_ref()).map_err(NoSession::Failed)?;
        let key = session::key(&client, &config.account)?;
        match client.scrobble(&key, plays) {
            Err(Failure::SessionExpired) => {
                session::forget();
                let key = session::key(&client, &config.account)?;
                client.scrobble(&key, plays).map_err(NoSession::Failed)
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
