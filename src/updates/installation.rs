//! How the running app was installed (S9.5.1), which decides how an update installs: a macOS
//! app bundle and an AppImage are replaced by the updater, a Windows Installer package upgrades
//! the app, and everything else (a Linux distribution's package, a Flatpak, a development
//! build) is updated by something else, so the updater only says a new version is out.

use std::path::{Path, PathBuf};

use super::manifest::PackageKind;

/// How the running app was installed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Installation {
    /// A macOS app bundle at this path.
    Bundle(PathBuf),
    /// A Linux AppImage at this path.
    AppImage(PathBuf),
    /// A Windows Installer package installed it.
    Msi,
    /// A package manager, Flatpak or a developer installs it.
    Managed,
}

impl Installation {
    /// How the running app was installed.
    pub fn detect() -> Self {
        let exe = std::env::current_exe().ok();
        let appimage = std::env::var_os("APPIMAGE").map(PathBuf::from);
        Self::detect_from(exe.as_deref(), appimage)
    }

    /// How an app whose executable is `exe` was installed; `appimage` is the `APPIMAGE`
    /// variable an AppImage's runtime sets.
    fn detect_from(exe: Option<&Path>, appimage: Option<PathBuf>) -> Self {
        if cfg!(target_os = "macos")
            && let Some(bundle) = exe.and_then(bundle_of)
        {
            return Self::Bundle(bundle);
        }
        if cfg!(target_os = "linux")
            && let Some(appimage) = appimage.filter(|path| path.is_absolute())
        {
            return Self::AppImage(appimage);
        }
        if cfg!(windows) && !cfg!(debug_assertions) && exe.is_some_and(in_program_files) {
            return Self::Msi;
        }
        Self::Managed
    }

    /// The kind of package that updates it; `None` when the updater can't.
    pub fn package_kind(&self) -> Option<PackageKind> {
        match self {
            Self::Bundle(_) => Some(PackageKind::App),
            Self::AppImage(_) => Some(PackageKind::AppImage),
            Self::Msi => Some(PackageKind::Msi),
            Self::Managed => None,
        }
    }

    /// What the updater replaces, for the kinds it replaces in place.
    pub fn replaced(&self) -> Option<&Path> {
        match self {
            Self::Bundle(path) | Self::AppImage(path) => Some(path),
            Self::Msi | Self::Managed => None,
        }
    }
}

/// The app bundle holding `exe` (`…/VLC.app/Contents/MacOS/RedesignedVLC`), if it is in one.
fn bundle_of(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    let is_bundle = macos.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && bundle.extension()? == "app";
    is_bundle.then(|| bundle.to_path_buf())
}

/// Whether `exe` is installed under Program Files, where the installer puts it.
fn in_program_files(exe: &Path) -> bool {
    ["ProgramFiles", "ProgramW6432"]
        .iter()
        .filter_map(std::env::var_os)
        .any(|folder| exe.starts_with(Path::new(&folder)))
}

/// Where an update of `replaced` is unpacked, and where the version it replaces is kept: next
/// to it, so both moves are renames on one disk, and hidden.
pub fn sibling(replaced: &Path, role: &str) -> Option<PathBuf> {
    let name = replaced.file_name()?.to_string_lossy();
    Some(replaced.with_file_name(format!(".{name}.{role}")))
}

/// Whether the folder holding `replaced` can be written, so it can be replaced.
pub fn can_replace(replaced: &Path) -> bool {
    let Some(probe) = sibling(replaced, "write-test") else { return false };
    let written = std::fs::write(&probe, b"");
    let _ = std::fs::remove_file(&probe);
    written.is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_bundle_on_macos_only() {
        let exe = Path::new("/Applications/VLC.app/Contents/MacOS/RedesignedVLC");
        let found = Installation::detect_from(Some(exe), None);
        if cfg!(target_os = "macos") {
            assert_eq!(found, Installation::Bundle("/Applications/VLC.app".into()));
            assert_eq!(found.package_kind(), Some(PackageKind::App));
        } else {
            assert_eq!(found, Installation::Managed);
        }
        let loose = Path::new("/Users/me/project/target/debug/RedesignedVLC");
        assert_eq!(Installation::detect_from(Some(loose), None), Installation::Managed);
    }

    #[test]
    fn finds_the_appimage_on_linux_only() {
        let appimage = Some(PathBuf::from("/home/me/Apps/VLC.AppImage"));
        let found =
            Installation::detect_from(Some(Path::new("/tmp/.mount_x/usr/bin/vlc")), appimage);
        if cfg!(target_os = "linux") {
            assert_eq!(found.replaced(), Some(Path::new("/home/me/Apps/VLC.AppImage")));
        } else {
            assert_eq!(found.package_kind(), None);
        }
    }

    #[test]
    fn siblings_are_hidden_beside_the_app() {
        let sibling = sibling(Path::new("/Applications/VLC.app"), "update");
        assert_eq!(sibling, Some(PathBuf::from("/Applications/.VLC.app.update")));
    }

    #[test]
    fn a_writable_folder_can_be_replaced_in() {
        let dir = std::env::temp_dir();
        assert!(can_replace(&dir.join("rvlc-replace-test.AppImage")));
        assert!(!can_replace(Path::new("/nonexistent/folder/VLC.app")));
    }
}
