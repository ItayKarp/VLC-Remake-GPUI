//! Updates (S9.5): with "Check for updates automatically" on (Settings → General, off by
//! default), the app reads a signed manifest over HTTPS once a day, downloads the new version's
//! package for this installation, checks its hash against the manifest, and installs it when
//! the app quits, so playback is never interrupted. The main window tells the user, with the
//! release notes a click away and a way to skip the version. An update that fails to start
//! twice is rolled back ([`rollback`]). How each part works is in `docs/adr/0005-updates.md`.
//!
//! The [`Updater`] global holds what the running app knows: whether a check is under way, the
//! update readied, and news for the main window to show once.

pub mod check;
pub mod install;
pub mod installation;
pub mod manifest;
pub mod rollback;
pub mod signing;
pub mod source;
pub mod state;

use std::path::PathBuf;
use std::time::Duration;

use gpui::{App, AsyncApp, Global};
use semver::Version;
use url::Url;

use self::install::Staged;
use self::installation::Installation;
use self::manifest::running_version;
use self::state::{Trial, UpdateState};
use crate::settings::{self, SettingsStore};

pub use rollback::launched;

/// How long an update must run before it counts as started (S9.5.2).
pub const STARTED_AFTER: Duration = Duration::from_secs(5);
/// How long after the app starts the first check waits, so it never slows the start.
const FIRST_CHECK_AFTER: Duration = Duration::from_secs(30);
/// How often the app looks at whether a check is due.
const TICK: Duration = Duration::from_hours(1);
/// How long a check counts for.
const CHECK_INTERVAL_S: i64 = 24 * 60 * 60;

/// A newer version, as the main window offers it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    /// The new version.
    pub version: Version,
    /// Its release notes, which also offer its downloads.
    pub notes_url: Url,
}

/// What the main window tells the user, once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum News {
    /// An update is ready and installs at quit.
    Ready(Offer),
    /// A newer version is out, which the updater can't install here.
    Available(Offer),
    /// This version came back because `0` failed to start.
    RolledBack(Version),
}

/// What the updater is doing.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Status {
    /// Nothing; a check may start.
    Idle,
    /// A check or download is under way.
    Busy,
    /// `version` is downloaded and readied: it installs at quit.
    Ready { version: Version, staged: Staged },
}

/// The updater's state while the app runs.
pub struct Updater {
    status: Status,
    news: Option<News>,
    /// The last version told about, so each is told once a run.
    announced: Option<Version>,
    installation: Installation,
    /// Where its files live; `None` turns updates off.
    data_dir: Option<PathBuf>,
    last_check_s: Option<i64>,
    /// Whether checking was on at the last look, to check as soon as it is turned on.
    enabled: bool,
}

impl Global for Updater {}

impl Updater {
    /// Holds `news` for the main window, unless its version was told about already.
    fn announce(&mut self, news: News) {
        let version = match &news {
            News::Ready(offer) | News::Available(offer) => Some(offer.version.clone()),
            News::RolledBack(_) => None,
        };
        if version.is_some() && version == self.announced {
            return;
        }
        self.announced = version.or(self.announced.take());
        self.news = Some(news);
    }

    /// Whether a check is due: checking is on, and the last check is a day old.
    fn is_due(&self) -> bool {
        let fresh = self.last_check_s.is_some_and(|last| state::now_s() - last < CHECK_INTERVAL_S);
        self.enabled && !fresh
    }
}

/// Starts the updater: ends a trial that starts fine, tells about a rollback, checks when due,
/// and installs a readied update at quit.
pub fn init(cx: &mut App) {
    let data_dir = state::data_dir();
    let mut saved = data_dir.as_deref().map_or_default(UpdateState::load);
    let rolled_back = saved.rolled_back.take();
    if let (Some(dir), Some(_)) = (&data_dir, &rolled_back) {
        saved.save(dir);
    }
    cx.set_global(Updater {
        status: Status::Idle,
        news: rolled_back.map(|news| News::RolledBack(news.version)),
        announced: None,
        installation: Installation::detect(),
        last_check_s: saved.last_check_s,
        enabled: settings::get(cx).prefs.startup.check_for_updates,
        data_dir,
    });
    confirm_start(saved.trial.as_ref(), cx);
    cx.spawn(async move |cx| check_when_due(cx).await).detach();
    cx.observe_global::<SettingsStore>(follow_setting).detach();
    cx.on_app_quit(|cx| {
        install_at_quit(cx);
        async {}
    })
    .detach();
}

/// Ends the trial of the update running once it has run for [`STARTED_AFTER`].
fn confirm_start(trial: Option<&Trial>, cx: &App) {
    if trial.is_none_or(|trial| trial.version != running_version()) {
        return;
    }
    let executor = cx.background_executor().clone();
    executor
        .clone()
        .spawn(async move {
            executor.timer(STARTED_AFTER).await;
            rollback::confirm();
        })
        .detach();
}

/// Checks a while after the start, then whenever a day has passed.
async fn check_when_due(cx: &mut AsyncApp) {
    cx.background_executor().timer(FIRST_CHECK_AFTER).await;
    loop {
        if cx.update(|cx| cx.try_global::<Updater>().is_some_and(Updater::is_due)) {
            check::run(cx).await;
        }
        cx.background_executor().timer(TICK).await;
    }
}

/// Checks as soon as "Check for updates automatically" is turned on.
fn follow_setting(cx: &mut App) {
    let enabled = settings::get(cx).prefs.startup.check_for_updates;
    let Some(updater) = cx.try_global::<Updater>() else { return };
    // Every settings change lands here; the global (whose observers redraw) changes only with
    // this one.
    if updater.enabled == enabled {
        return;
    }
    cx.global_mut::<Updater>().enabled = enabled;
    if enabled && cx.global::<Updater>().is_due() {
        cx.spawn(async move |cx| check::run(cx).await).detach();
    }
}

/// Takes the news the main window hasn't shown yet.
pub fn take_news(cx: &mut App) -> Option<News> {
    // Changing the global notifies its observers, the main window among them, so it is changed
    // only when there is news to take.
    cx.try_global::<Updater>()?.news.as_ref()?;
    cx.global_mut::<Updater>().news.take()
}

/// Skips `version`: it isn't offered again, and if it is readied it won't be installed.
pub fn skip(version: &Version, cx: &mut App) {
    let Some(updater) = cx.try_global::<Updater>() else { return };
    let Some(dir) = updater.data_dir.clone() else { return };
    if let Status::Ready { version: ready, staged } = &updater.status
        && ready == version
    {
        install::discard_staged(staged);
        cx.global_mut::<Updater>().status = Status::Idle;
    }
    let mut state = UpdateState::load(&dir);
    state.skipped = state.skipped.take().max(Some(version.clone()));
    state.save(&dir);
}

/// Installs the readied update as the app quits; it is on trial from its first start.
fn install_at_quit(cx: &mut App) {
    // A clean quit means this version started, if it was on trial.
    rollback::confirm();
    let Some(updater) = cx.try_global::<Updater>() else { return };
    let (Status::Ready { version, staged }, Some(dir)) = (&updater.status, &updater.data_dir)
    else {
        return;
    };
    let running = running_version();
    match install::install(staged, &running, dir) {
        Ok(backup) => {
            tracing::info!("updates: installed {version}; it runs from the next start");
            let mut state = UpdateState::load(dir);
            let trial = Trial { version: version.clone(), previous: running, starts: 0, backup };
            state.trial = Some(trial);
            state.save(dir);
        }
        Err(error) => tracing::error!("updates: couldn't install {version}: {error}"),
    }
}
