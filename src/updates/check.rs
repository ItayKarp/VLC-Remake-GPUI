//! One check for updates (S9.5.1): reads the signed manifest, and when it offers a newer version
//! than the running one (and one the user didn't skip), downloads this installation's package,
//! checks it and readies it to install at quit. When the updater can't install it (a package
//! manager's install, or a folder it can't write to), it only says the version is out.
//!
//! Everything slow runs off the UI thread, and nothing here touches playback: an update is only
//! ever installed when the app quits. Failures (no internet, a proxy refusing, a bad download)
//! are logged and left to the next check.

use std::path::{Path, PathBuf};

use gpui::AsyncApp;
use semver::Version;
use url::Url;

use super::install::{self, Staged};
use super::installation::{self, Installation};
use super::manifest::{Manifest, Package, running_version};
use super::source::{self, Failure};
use super::state::{self, UpdateState};
use super::{News, Offer, Status, Updater};
use crate::settings;

/// What a check found.
enum Found {
    /// Nothing newer, or nothing not skipped.
    Nothing,
    /// A newer version the updater can't install.
    Available(Offer),
    /// A newer version, downloaded and ready to install at quit.
    Ready(Offer, Staged),
}

/// Checks for an update, unless one is being checked for or is ready already.
pub async fn run(cx: &mut AsyncApp) {
    let started = cx.update(|cx| {
        let updater = cx.try_global::<Updater>()?;
        let idle = matches!(updater.status, Status::Idle);
        let dir = updater.data_dir.clone().filter(|_| idle)?;
        let proxy = settings::proxy_url(&settings::get(cx).prefs.network.http_proxy);
        let installation = updater.installation.clone();
        cx.global_mut::<Updater>().status = Status::Busy;
        Some((dir, proxy, installation))
    });
    let Some((dir, proxy, installation)) = started else { return };
    let found = cx
        .background_executor()
        .spawn(async move { find(&dir, proxy.as_ref(), &installation) })
        .await;
    cx.update(|cx| {
        let updater = cx.global_mut::<Updater>();
        updater.last_check_s = Some(state::now_s());
        updater.status = Status::Idle;
        match found {
            Ok(Found::Nothing) => {}
            Ok(Found::Available(offer)) => updater.announce(News::Available(offer)),
            Ok(Found::Ready(offer, staged)) => {
                updater.status = Status::Ready { version: offer.version.clone(), staged };
                updater.announce(News::Ready(offer));
            }
            Err(failure) => tracing::info!("updates: the check failed: {failure}"),
        }
        cx.refresh_windows();
    });
}

/// The check itself, blocking: reads the manifest, then downloads and readies its package.
fn find(dir: &Path, proxy: Option<&Url>, installation: &Installation) -> Result<Found, Failure> {
    let url = source::manifest_url();
    tracing::info!("updates: checking {url}");
    let fetched = source::fetch_manifest(&url, proxy);
    let mut state = UpdateState::load(dir);
    state.last_check_s = Some(state::now_s());
    state.save(dir);
    let manifest = fetched?;
    if !manifest.is_offered(&running_version(), state.skipped.as_ref()) {
        return Ok(Found::Nothing);
    }
    let offer = Offer { version: manifest.version.clone(), notes_url: manifest.notes_url.clone() };
    match installable(&manifest, installation) {
        Some(package) => {
            let staged = ready(package, &manifest.version, dir, proxy, installation)?;
            Ok(Found::Ready(offer, staged))
        }
        None => Ok(Found::Available(offer)),
    }
}

/// Downloads `package` of `version` into `dir` (or beside the app), checks it and readies it.
fn ready(
    package: &Package,
    version: &Version,
    dir: &Path,
    proxy: Option<&Url>,
    installation: &Installation,
) -> Result<Staged, Failure> {
    let download = install::download_path(installation, package, version, dir);
    std::fs::create_dir_all(dir).map_err(Failure::Disk)?;
    tracing::info!("updates: downloading {version} from {}", package.url);
    source::download(package, &download, proxy)?;
    let staged = install::stage(installation, &download).map_err(Failure::Disk)?;
    forget_older_downloads(dir, &download, version);
    Ok(staged)
}

/// The package that updates `installation`, if the updater can install it there.
fn installable<'a>(manifest: &'a Manifest, installation: &Installation) -> Option<&'a Package> {
    let kind = installation.package_kind()?;
    let package = manifest.package(kind, std::env::consts::OS, std::env::consts::ARCH)?;
    let writable = installation.replaced().is_none_or(installation::can_replace);
    if !writable {
        tracing::info!("updates: the app's folder can't be written to, so it isn't updated");
    }
    writable.then_some(package)
}

/// Removes downloads in `dir` other than `kept` and the packages kept for going back: leftovers
/// of updates that were never installed.
fn forget_older_downloads(dir: &Path, kept: &Path, newest: &Version) {
    let running = install::msi_name(&running_version());
    let keep = |path: &PathBuf| {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        path == kept
            || name == running
            || name.ends_with(".toml")
            || name.contains(&format!("-{newest}-"))
    };
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for path in entries.filter_map(Result::ok).map(|entry| entry.path()) {
        if path.is_file() && !keep(&path) {
            let _ = std::fs::remove_file(&path);
        }
    }
}
