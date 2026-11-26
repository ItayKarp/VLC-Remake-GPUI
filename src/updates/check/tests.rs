//! The check against a stand-in [`UpdateSource`]: each outcome, what it saves, and what the
//! [`Updater`] hears.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use gpui::TestAppContext;
use url::Url;

use super::*;
use crate::updates::manifest::PackageKind;

/// A source serving `manifest` (failing without one) and writing `package` for a download
/// (failing as corrupt without one), counting what it is asked.
struct Stand {
    manifest: Option<Manifest>,
    package: Option<Vec<u8>>,
    manifest_reads: Arc<AtomicUsize>,
    downloads: Arc<AtomicUsize>,
}

impl Stand {
    fn new(manifest: Option<Manifest>) -> Self {
        Self {
            manifest,
            package: Some(b"package".to_vec()),
            manifest_reads: Arc::default(),
            downloads: Arc::default(),
        }
    }
}

impl UpdateSource for Stand {
    fn location(&self) -> &'static str {
        "https://example.com/update-manifest.json"
    }

    fn manifest(&self) -> Result<Manifest, Failure> {
        self.manifest_reads.fetch_add(1, Ordering::SeqCst);
        self.manifest.clone().ok_or_else(|| Failure::Network("offline".into()))
    }

    fn download(&self, _: &Package, path: &Path) -> Result<(), Failure> {
        self.downloads.fetch_add(1, Ordering::SeqCst);
        let bytes = self.package.as_ref().ok_or(Failure::Corrupt)?;
        std::fs::write(path, bytes).map_err(Failure::Disk)
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rvlc-update-check-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn version(text: &str) -> Version {
    Version::parse(text).unwrap()
}

/// A manifest of `version` with a Windows Installer package for this computer.
fn manifest(version_text: &str) -> Manifest {
    let url = |path: &str| Url::parse(&format!("https://example.com/{path}")).unwrap();
    Manifest {
        version: version(version_text),
        notes_url: url("notes"),
        packages: vec![Package {
            os: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
            kind: PackageKind::Msi,
            url: url("vlc.msi"),
            size: 7,
            sha256: "ab".into(),
        }],
    }
}

fn offer(of: &Manifest) -> Offer {
    Offer { version: of.version.clone(), notes_url: of.notes_url.clone() }
}

#[test]
fn the_running_version_is_not_offered_but_the_check_is_saved() {
    let dir = temp_dir("same");
    let running = running_version().to_string();
    let found = find(&Stand::new(Some(manifest(&running))), &dir, &Installation::Msi);
    assert_eq!(found.unwrap(), Found::Nothing);
    assert!(UpdateState::load(&dir).last_check_s.is_some());
}

#[test]
fn a_failed_check_is_still_saved_so_it_waits_a_day() {
    let dir = temp_dir("offline");
    let found = find(&Stand::new(None), &dir, &Installation::Msi);
    assert!(matches!(found, Err(Failure::Network(_))));
    assert!(UpdateState::load(&dir).last_check_s.is_some());
}

#[test]
fn a_skipped_version_is_not_offered() {
    let dir = temp_dir("skipped");
    let state = UpdateState { skipped: Some(version("99.0.0")), ..UpdateState::default() };
    state.save(&dir);
    let found = find(&Stand::new(Some(manifest("99.0.0"))), &dir, &Installation::Msi);
    assert_eq!(found.unwrap(), Found::Nothing);
}

#[test]
fn a_managed_install_only_hears_of_the_new_version() {
    let dir = temp_dir("managed");
    let newer = manifest("99.0.0");
    let source = Stand::new(Some(newer.clone()));
    let found = find(&source, &dir, &Installation::Managed);
    assert_eq!(found.unwrap(), Found::Available(offer(&newer)));
    assert_eq!(source.downloads.load(Ordering::SeqCst), 0, "nothing is downloaded");
}

#[test]
fn an_installer_package_is_downloaded_and_readied() {
    let dir = temp_dir("msi");
    let newer = manifest("99.0.0");
    let source = Stand::new(Some(newer.clone()));
    let found = find(&source, &dir, &Installation::Msi).unwrap();
    let path = dir.join(install::msi_name(&newer.version));
    assert_eq!(found, Found::Ready(offer(&newer), Staged::Msi(path.clone())));
    assert_eq!(std::fs::read(path).unwrap(), b"package");
}

#[test]
fn a_bad_download_readies_nothing() {
    let dir = temp_dir("corrupt");
    let source = Stand { package: None, ..Stand::new(Some(manifest("99.0.0"))) };
    let found = find(&source, &dir, &Installation::Msi);
    assert!(matches!(found, Err(Failure::Corrupt)));
}

#[test]
fn readying_an_update_forgets_older_downloads_but_keeps_the_way_back() {
    let dir = temp_dir("forget");
    let running_package = install::msi_name(&running_version());
    for name in ["old-update.zip", "updates.toml", running_package.as_str()] {
        std::fs::write(dir.join(name), "x").unwrap();
    }
    find(&Stand::new(Some(manifest("99.0.0"))), &dir, &Installation::Msi).unwrap();
    assert!(!dir.join("old-update.zip").exists(), "a leftover download goes");
    assert!(dir.join("updates.toml").exists(), "the updater's state stays");
    assert!(dir.join(running_package).exists(), "the running version's package stays");
}

fn updater(dir: &Path, status: Status) -> Updater {
    Updater {
        status,
        news: None,
        announced: None,
        installation: Installation::Msi,
        data_dir: Some(dir.to_path_buf()),
        last_check_s: None,
        enabled: true,
    }
}

/// Runs a check of `source` with the updater in `status`, until it settles.
fn run_check(source: Stand, status: Status, name: &str, cx: &mut TestAppContext) {
    let dir = temp_dir(name);
    cx.update(|cx| cx.set_global(updater(&dir, status)));
    cx.update(|cx| cx.spawn(async move |cx| run_with(source, cx).await).detach());
    cx.run_until_parked();
}

#[gpui::test]
fn a_readied_update_is_held_and_announced(cx: &mut TestAppContext) {
    let newer = manifest("99.0.0");
    run_check(Stand::new(Some(newer.clone())), Status::Idle, "announce", cx);
    cx.update(|cx| {
        let updater = cx.global::<Updater>();
        assert!(
            matches!(&updater.status, Status::Ready { version, .. } if *version == newer.version)
        );
        assert_eq!(updater.news, Some(News::Ready(offer(&newer))));
        assert!(updater.last_check_s.is_some());
    });
}

#[gpui::test]
fn a_newer_version_it_cannot_install_is_announced_as_available(cx: &mut TestAppContext) {
    let newer = manifest("99.0.0");
    let source = Stand::new(Some(Manifest { packages: Vec::new(), ..newer.clone() }));
    run_check(source, Status::Idle, "available", cx);
    cx.update(|cx| {
        let updater = cx.global::<Updater>();
        assert_eq!(updater.status, Status::Idle);
        assert_eq!(updater.news, Some(News::Available(offer(&newer))));
    });
}

#[gpui::test]
fn a_failed_check_leaves_the_updater_idle_and_quiet(cx: &mut TestAppContext) {
    run_check(Stand::new(None), Status::Idle, "quiet", cx);
    cx.update(|cx| {
        let updater = cx.global::<Updater>();
        assert_eq!(updater.status, Status::Idle);
        assert_eq!(updater.news, None);
        assert!(updater.last_check_s.is_some(), "it waits a day before trying again");
    });
}

#[gpui::test]
fn no_check_starts_while_one_is_under_way(cx: &mut TestAppContext) {
    let source = Stand::new(Some(manifest("99.0.0")));
    let reads = source.manifest_reads.clone();
    run_check(source, Status::Busy, "busy", cx);
    assert_eq!(reads.load(Ordering::SeqCst), 0);
    cx.update(|cx| assert_eq!(cx.global::<Updater>().status, Status::Busy));
}
