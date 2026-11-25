use std::fs;

use url::Url;

use super::*;
use crate::opening::scratch::scratch_dir;
use crate::updates::launcher::tests::Recorder;
use crate::updates::manifest::PackageKind;

fn package(url: &str) -> Package {
    Package {
        os: "linux".into(),
        arch: "x86_64".into(),
        kind: PackageKind::AppImage,
        url: Url::parse(url).unwrap(),
        size: 1,
        sha256: String::new(),
    }
}

#[test]
fn an_appimage_is_swapped_in_and_can_be_restored() {
    let dir = scratch_dir();
    let appimage = dir.join("VLC.AppImage");
    fs::write(&appimage, "old").unwrap();
    let installation = Installation::AppImage(appimage.clone());
    let version = Version::new(0, 2, 0);
    let download = download_path(&installation, &package("https://e.com/a"), &version, &dir);
    assert_eq!(download, dir.join(".VLC.AppImage.update"));
    fs::write(&download, "new").unwrap();

    let staged = stage(&installation, &download).unwrap();
    let backup = install(&staged, &Version::new(0, 1, 0), &dir, &Recorder::default()).unwrap();
    assert_eq!(fs::read_to_string(&appimage).unwrap(), "new");
    assert!(!download.exists());

    assert_eq!(restore(&backup, &Recorder::default()).unwrap(), Some(appimage.clone()));
    assert_eq!(fs::read_to_string(&appimage).unwrap(), "old");
    assert!(!dir.join(".VLC.AppImage.failed").exists(), "the failed update is thrown away");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_bundle_folder_is_swapped_and_the_previous_one_discarded() {
    let dir = scratch_dir();
    let installed = dir.join("VLC.app");
    fs::create_dir_all(installed.join("Contents")).unwrap();
    let scratch = dir.join(".VLC.app.update");
    let ready = scratch.join("VLC.app");
    fs::create_dir_all(ready.join("Contents/New")).unwrap();
    let staged = Staged::Moved { installed: installed.clone(), ready, scratch: scratch.clone() };

    let backup = install(&staged, &Version::new(0, 1, 0), &dir, &Recorder::default()).unwrap();
    assert!(installed.join("Contents/New").is_dir());
    assert!(!scratch.exists());
    let Backup::Moved { saved, .. } = &backup else { panic!("a bundle is moved") };
    assert!(saved.join("Contents").is_dir());

    discard(&backup);
    assert!(!saved.exists());
    assert!(installed.join("Contents/New").is_dir());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_failed_swap_leaves_the_app_in_place() {
    let dir = scratch_dir();
    let installed = dir.join("VLC.AppImage");
    fs::write(&installed, "old").unwrap();
    let missing = dir.join("gone");
    let staged =
        Staged::Moved { installed: installed.clone(), ready: missing.clone(), scratch: missing };
    assert!(install(&staged, &Version::new(0, 1, 0), &dir, &Recorder::default()).is_err());
    assert_eq!(fs::read_to_string(&installed).unwrap(), "old");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn packages_download_into_the_data_folder_by_kind() {
    let dir = Path::new("/data/updates");
    let version = Version::new(1, 2, 3);
    let zip = package("https://e.com/download/v1.2.3/RedesignedVLC-1.2.3-macos-arm64.app.zip");
    let bundle = Installation::Bundle("/Applications/VLC.app".into());
    let path = download_path(&bundle, &zip, &version, dir);
    assert_eq!(path, dir.join("RedesignedVLC-1.2.3-macos-arm64.app.zip"));
    let msi = download_path(&Installation::Msi, &package("https://e.com/x.msi"), &version, dir);
    assert_eq!(msi, dir.join("RedesignedVLC-1.2.3-windows-x64.msi"));
}

#[test]
fn without_the_previous_package_an_msi_cannot_go_back() {
    let backup = Backup::Msi { installed: "new.msi".into(), previous: None };
    assert!(restore(&backup, &Recorder::default()).is_err());
}

#[test]
fn an_msi_is_installed_by_msiexec_keeping_the_previous_package() {
    let dir = scratch_dir();
    let running = Version::new(0, 1, 0);
    let package = dir.join(msi_name(&Version::new(0, 2, 0)));
    let staged = stage(&Installation::Msi, &package).unwrap();
    assert_eq!(staged, Staged::Msi(package.clone()));
    let launcher = Recorder::default();

    let backup = install(&staged, &running, &dir, &launcher).unwrap();
    assert_eq!(backup, Backup::Msi { installed: package.clone(), previous: None });
    let shown = package.to_string_lossy().into_owned();
    assert_eq!(launcher.launched(), [["msiexec", "/i", &shown, "/passive"]]);

    let previous = dir.join(msi_name(&running));
    fs::write(&previous, "previous").unwrap();
    let backup = install(&staged, &running, &dir, &launcher).unwrap();
    assert_eq!(backup, Backup::Msi { installed: package, previous: Some(previous) });
}

#[test]
fn an_msi_whose_installer_does_not_start_is_not_installed() {
    let staged = Staged::Msi("new.msi".into());
    let installed = install(&staged, &Version::new(0, 1, 0), &scratch_dir(), &Recorder::failing());
    assert_eq!(installed.unwrap_err().kind(), io::ErrorKind::NotFound);
}

#[test]
fn the_previous_msi_is_reinstalled_after_this_process_exits_on_windows_only() {
    let backup = Backup::Msi { installed: "new.msi".into(), previous: Some("old.msi".into()) };
    let launcher = Recorder::default();
    let restored = restore(&backup, &launcher);
    if cfg!(windows) {
        assert_eq!(restored.unwrap(), None, "the installer starts it");
        assert_eq!(launcher.launched()[0][0], "cmd");
    } else {
        assert_eq!(restored.unwrap_err().kind(), io::ErrorKind::Unsupported);
        assert_eq!(launcher.launched(), Vec::<Vec<String>>::new());
    }
}

#[test]
fn a_managed_installation_is_not_staged() {
    let staged = stage(&Installation::Managed, Path::new("update"));
    assert_eq!(staged.unwrap_err().kind(), io::ErrorKind::Unsupported);
    let dir = Path::new("/data/updates");
    let nameless = download_path(
        &Installation::Managed,
        &package("https://e.com/"),
        &Version::new(1, 0, 0),
        dir,
    );
    assert_eq!(nameless, dir.join("update"));
}

#[test]
fn a_bundle_zip_that_does_not_unzip_is_thrown_away() {
    let dir = scratch_dir();
    let bundle = dir.join("VLC.app");
    fs::create_dir_all(&bundle).unwrap();
    let zip = dir.join("update.app.zip");
    fs::write(&zip, "not a zip").unwrap();
    assert!(stage(&Installation::Bundle(bundle.clone()), &zip).is_err());
    assert!(!zip.exists());
    assert!(bundle.is_dir(), "the app stays");
}

#[test]
fn an_app_without_a_name_cannot_be_swapped() {
    let staged =
        Staged::Moved { installed: "/".into(), ready: "ready".into(), scratch: "s".into() };
    let swapped = install(&staged, &Version::new(0, 1, 0), &scratch_dir(), &Recorder::default());
    assert_eq!(swapped.unwrap_err().kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn a_missing_previous_version_cannot_be_restored_and_the_update_stays() {
    let dir = scratch_dir();
    let installed = dir.join("VLC.AppImage");
    fs::write(&installed, "new").unwrap();
    let backup = Backup::Moved { installed: installed.clone(), saved: dir.join("gone") };
    assert!(restore(&backup, &Recorder::default()).is_err());
    assert_eq!(fs::read_to_string(&installed).unwrap(), "new", "the app is still there");
    assert!(!dir.join(".VLC.AppImage.failed").exists());
}

#[test]
fn an_app_with_no_name_cannot_be_restored() {
    let backup = Backup::Moved { installed: "/".into(), saved: scratch_dir().join("saved") };
    let restored = restore(&backup, &Recorder::default());
    assert_eq!(restored.unwrap_err().kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn a_started_msi_update_throws_the_previous_package_away() {
    let dir = scratch_dir();
    let previous = dir.join("old.msi");
    fs::write(&previous, "old").unwrap();
    discard(&Backup::Msi { installed: "new.msi".into(), previous: Some(previous.clone()) });
    assert!(!previous.exists());
    discard(&Backup::Msi { installed: "new.msi".into(), previous: None });
    discard(&Backup::Msi { installed: "new.msi".into(), previous: Some(previous) });
}

#[test]
fn a_skipped_update_is_thrown_away() {
    let dir = scratch_dir();
    let scratch = dir.join(".VLC.app.update");
    fs::create_dir_all(scratch.join("VLC.app")).unwrap();
    let ready = scratch.join("VLC.app");
    discard_staged(&Staged::Moved {
        installed: dir.join("VLC.app"),
        ready,
        scratch: scratch.clone(),
    });
    assert!(!scratch.exists());

    let package = dir.join("new.msi");
    fs::write(&package, "new").unwrap();
    discard_staged(&Staged::Msi(package.clone()));
    assert!(!package.exists());
}

#[cfg(unix)]
#[test]
fn a_staged_appimage_can_run() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch_dir();
    let download = dir.join(".VLC.AppImage.update");
    fs::write(&download, "new").unwrap();
    stage(&Installation::AppImage(dir.join("VLC.AppImage")), &download).unwrap();
    assert_eq!(fs::metadata(&download).unwrap().permissions().mode() & 0o777, 0o755);
}
