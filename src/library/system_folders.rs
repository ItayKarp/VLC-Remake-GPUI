//! The system and app folders the library never indexes (S5.9.3): when a whole drive or the
//! home folder is a media folder, the OS's own folders (`Windows`, `/usr`, `~/Library`,
//! `AppData`, …) hold no one's media, only thousands of files to walk.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Folders at the top of a drive that belong to the OS or its apps.
const AT_DRIVE_ROOT: &[&str] = &[
    "$Recycle.Bin",
    "Applications",
    "bin",
    "boot",
    "cores",
    "dev",
    "etc",
    "Library",
    "lib",
    "lib32",
    "lib64",
    "lost+found",
    "opt",
    "private",
    "proc",
    "Program Files",
    "Program Files (x86)",
    "ProgramData",
    "Recovery",
    "run",
    "sbin",
    "snap",
    "srv",
    "sys",
    "System",
    "System Volume Information",
    "tmp",
    "usr",
    "var",
    "Volumes",
    "Windows",
];

/// Folders in the home folder that hold app data rather than the user's files.
const IN_HOME: &[&str] =
    &[".cache", ".config", ".local", ".var", "AppData", "Applications", "Library", "snap"];

/// Whether the library skips the folder at `path`: a system folder at the top of a drive or in
/// the home folder.
pub fn is_system_folder(path: &Path) -> bool {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else { return false };
    let name = name.to_string_lossy();
    let listed = |names: &[&str]| names.iter().any(|known| known.eq_ignore_ascii_case(&name));
    (parent.parent().is_none() && listed(AT_DRIVE_ROOT))
        || (home().is_some_and(|home| home == parent) && listed(IN_HOME))
}

/// The home folder, read once.
fn home() -> Option<&'static Path> {
    static HOME: OnceLock<Option<PathBuf>> = OnceLock::new();
    HOME.get_or_init(dirs::home_dir).as_deref()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_os_folders_of_a_drive_and_home_are_skipped() {
        assert!(is_system_folder(Path::new("/usr")));
        assert!(is_system_folder(Path::new("/System")));
        assert!(!is_system_folder(Path::new("/Users")));
        assert!(!is_system_folder(Path::new("/media/usr")));
        if let Some(home) = home() {
            assert!(is_system_folder(&home.join("Library")));
            assert!(!is_system_folder(&home.join("Movies")));
        }
    }
}
