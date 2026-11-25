//! Putting an update in place (S9.5): readying a downloaded package, installing it when the app
//! quits, and putting the previous version back if the update fails to start (S9.5.2).
//!
//! An app bundle or AppImage is readied beside the running one and swapped in with two renames
//! at quit, the previous version kept beside it until the update has started. A Windows
//! Installer package is handed to `msiexec` at quit; the packages the updater installs are kept,
//! so the previous version's can be installed again.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use semver::Version;

use super::installation::{self, Installation};
use super::launcher::Launcher;
use super::manifest::Package;
use super::state::Backup;

/// What a sibling of the app holding a readied update is called.
const UPDATE_ROLE: &str = "update";
/// What a sibling of the app holding the previous version is called.
const PREVIOUS_ROLE: &str = "previous";

/// An update downloaded, checked and ready to install at quit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Staged {
    /// `ready` takes the place of `installed`; `scratch` is removed afterwards.
    Moved {
        /// The app being replaced.
        installed: PathBuf,
        /// The new version, beside it.
        ready: PathBuf,
        /// What to remove once it is in place (or skipped).
        scratch: PathBuf,
    },
    /// A Windows Installer package.
    Msi(PathBuf),
}

/// Where `package`, of version `version`, is downloaded to for `installation`: beside an
/// AppImage (which is ready as it lands), into `data_dir` otherwise, a Windows Installer
/// package under the name it is kept by.
pub fn download_path(
    installation: &Installation,
    package: &Package,
    version: &Version,
    data_dir: &Path,
) -> PathBuf {
    match installation {
        Installation::AppImage(path) => installation::sibling(path, UPDATE_ROLE)
            .unwrap_or_else(|| data_dir.join("update.AppImage")),
        Installation::Msi => data_dir.join(msi_name(version)),
        Installation::Bundle(_) | Installation::Managed => data_dir.join(file_name(package)),
    }
}

/// The name of `package`'s file: the last part of its address.
fn file_name(package: &Package) -> String {
    let segment = package.url.path_segments().and_then(Iterator::last).unwrap_or_default();
    if segment.is_empty() { "update".to_owned() } else { segment.to_owned() }
}

/// Readies the package downloaded to `download` for `installation`.
pub fn stage(installation: &Installation, download: &Path) -> io::Result<Staged> {
    match installation {
        Installation::Bundle(bundle) => stage_bundle(bundle, download),
        Installation::AppImage(appimage) => {
            make_executable(download)?;
            let ready = download.to_path_buf();
            Ok(Staged::Moved { installed: appimage.clone(), scratch: ready.clone(), ready })
        }
        Installation::Msi => Ok(Staged::Msi(download.to_path_buf())),
        Installation::Managed => Err(io::Error::from(io::ErrorKind::Unsupported)),
    }
}

/// Unzips the app in `zip` beside `bundle` with `ditto`, which keeps its signature intact.
fn stage_bundle(bundle: &Path, zip: &Path) -> io::Result<Staged> {
    let scratch = installation::sibling(bundle, UPDATE_ROLE).ok_or(io::ErrorKind::InvalidInput)?;
    remove(&scratch)?;
    std::fs::create_dir_all(&scratch)?;
    let unzipped = Command::new("ditto").arg("-x").arg("-k").arg(zip).arg(&scratch).status();
    let _ = std::fs::remove_file(zip);
    if !unzipped?.success() {
        return Err(io::Error::other("ditto couldn't unzip the update"));
    }
    let ready = std::fs::read_dir(&scratch)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|extension| extension == "app"))
        .ok_or_else(|| io::Error::other("the update holds no app"))?;
    Ok(Staged::Moved { installed: bundle.to_path_buf(), ready, scratch })
}

/// Installs `staged`, which replaces `running`; answers how to go back. `data_dir` holds the
/// Windows Installer packages kept; `launcher` starts the installer.
pub fn install(
    staged: &Staged,
    running: &Version,
    data_dir: &Path,
    launcher: &dyn Launcher,
) -> io::Result<Backup> {
    match staged {
        Staged::Moved { installed, ready, scratch } => {
            let saved = swap(installed, ready)?;
            let _ = remove(scratch);
            Ok(Backup::Moved { installed: installed.clone(), saved })
        }
        Staged::Msi(package) => {
            let mut msiexec = Command::new("msiexec");
            msiexec.arg("/i").arg(package).arg("/passive");
            launcher.launch(msiexec)?;
            let previous = data_dir.join(msi_name(running));
            let previous = previous.is_file().then_some(previous);
            Ok(Backup::Msi { installed: package.clone(), previous })
        }
    }
}

/// The name the updater keeps version `version`'s Windows Installer package under.
pub fn msi_name(version: &Version) -> String {
    format!("RedesignedVLC-{version}-windows-x64.msi")
}

/// Moves `installed` aside and `ready` into its place; answers where `installed` went. Puts it
/// back if the second move fails.
fn swap(installed: &Path, ready: &Path) -> io::Result<PathBuf> {
    let saved =
        installation::sibling(installed, PREVIOUS_ROLE).ok_or(io::ErrorKind::InvalidInput)?;
    remove(&saved)?;
    std::fs::rename(installed, &saved)?;
    if let Err(error) = std::fs::rename(ready, installed) {
        std::fs::rename(&saved, installed)?;
        return Err(error);
    }
    Ok(saved)
}

/// Puts back the version `backup` kept, replacing the update. Answers the program to start it
/// with, unless it is started already. `launcher` starts the Windows Installer.
pub fn restore(backup: &Backup, launcher: &dyn Launcher) -> io::Result<Option<PathBuf>> {
    match backup {
        Backup::Moved { installed, saved } => {
            remove(installed)?;
            std::fs::rename(saved, installed)?;
            // An AppImage is its own program; a bundle's is where this one ran from.
            let program =
                if installed.is_file() { installed.clone() } else { std::env::current_exe()? };
            Ok(Some(program))
        }
        Backup::Msi { installed, previous: Some(previous) } => {
            reinstall_after_exit(installed, previous, launcher)?;
            Ok(None)
        }
        Backup::Msi { previous: None, .. } => Err(io::Error::other("no previous package kept")),
    }
}

/// The update started: the version kept for going back isn't needed any more.
pub fn discard(backup: &Backup) {
    let removed = match backup {
        Backup::Moved { saved, .. } => remove(saved),
        Backup::Msi { previous: Some(previous), .. } => std::fs::remove_file(previous),
        Backup::Msi { previous: None, .. } => Ok(()),
    };
    if let Err(error) = removed {
        tracing::warn!("updates: couldn't remove the previous version: {error}");
    }
}

/// Throws a readied update away (the user skipped it).
pub fn discard_staged(staged: &Staged) {
    let path = match staged {
        Staged::Moved { scratch, .. } => scratch,
        Staged::Msi(package) => package,
    };
    if let Err(error) = remove(path) {
        tracing::warn!("updates: couldn't remove the skipped update: {error}");
    }
}

/// Removes the file or folder at `path`, if there is one.
fn remove(path: &Path) -> io::Result<()> {
    let removed =
        if path.is_dir() { std::fs::remove_dir_all(path) } else { std::fs::remove_file(path) };
    match removed {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn make_executable(_: &Path) -> io::Result<()> {
    Ok(())
}

/// Once this process has quit, uninstalls the update with its package and installs the
/// previous version's, then starts it. `cmd` waits two seconds first, so the update's files are
/// no longer in use.
#[cfg(windows)]
fn reinstall_after_exit(
    installed: &Path,
    previous: &Path,
    launcher: &dyn Launcher,
) -> io::Result<()> {
    use std::os::windows::process::CommandExt;
    let exe = std::env::current_exe()?;
    let commands = format!(
        "timeout /t 2 /nobreak >nul & msiexec /x \"{}\" /passive && msiexec /i \"{}\" /passive \
         && start \"\" \"{}\"",
        installed.display(),
        previous.display(),
        exe.display()
    );
    let mut cmd = Command::new("cmd");
    cmd.raw_arg(format!("/C \"{commands}\""));
    launcher.launch(cmd)
}

#[cfg(not(windows))]
fn reinstall_after_exit(_: &Path, _: &Path, _: &dyn Launcher) -> io::Result<()> {
    Err(io::Error::from(io::ErrorKind::Unsupported))
}

#[cfg(test)]
mod tests;
