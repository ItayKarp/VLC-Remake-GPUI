use std::fs;

use url::Url;

use super::*;
use crate::updates::manifest::PackageKind;

/// A fresh folder for one test.
fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rvlc-install-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

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
    let dir = scratch_dir("appimage");
    let appimage = dir.join("VLC.AppImage");
    fs::write(&appimage, "old").unwrap();
    let installation = Installation::AppImage(appimage.clone());
    let version = Version::new(0, 2, 0);
    let download = download_path(&installation, &package("https://e.com/a"), &version, &dir);
    assert_eq!(download, dir.join(".VLC.AppImage.update"));
    fs::write(&download, "new").unwrap();

    let staged = stage(&installation, &download).unwrap();
    let backup = install(&staged, &Version::new(0, 1, 0), &dir).unwrap();
    assert_eq!(fs::read_to_string(&appimage).unwrap(), "new");
    assert!(!download.exists());

    assert_eq!(restore(&backup).unwrap(), Some(appimage.clone()));
    assert_eq!(fs::read_to_string(&appimage).unwrap(), "old");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_bundle_folder_is_swapped_and_the_previous_one_discarded() {
    let dir = scratch_dir("bundle");
    let installed = dir.join("VLC.app");
    fs::create_dir_all(installed.join("Contents")).unwrap();
    let scratch = dir.join(".VLC.app.update");
    let ready = scratch.join("VLC.app");
    fs::create_dir_all(ready.join("Contents/New")).unwrap();
    let staged = Staged::Moved { installed: installed.clone(), ready, scratch: scratch.clone() };

    let backup = install(&staged, &Version::new(0, 1, 0), &dir).unwrap();
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
    let dir = scratch_dir("failed");
    let installed = dir.join("VLC.AppImage");
    fs::write(&installed, "old").unwrap();
    let missing = dir.join("gone");
    let staged =
        Staged::Moved { installed: installed.clone(), ready: missing.clone(), scratch: missing };
    assert!(install(&staged, &Version::new(0, 1, 0), &dir).is_err());
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
    assert!(restore(&backup).is_err());
}
