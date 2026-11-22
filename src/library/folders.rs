//! Managing the media folders from Settings → Library (S5.9.3): adding one, de-duplicated by its
//! canonical path, and removing one.
//!
//! - The same folder twice, through a symlink or another spelling, is refused.
//! - A folder inside one the library already has is refused: it is indexed already.
//! - A folder holding some the library has takes their place, so nothing is indexed twice.
//! - A whole drive or the home folder is [`broad`]: the page asks first, since indexing it takes
//!   long. The walker skips the system folders inside it ([`system_folders`](super::system_folders)).

use std::fs;
use std::path::{Path, PathBuf};

use crate::db::roots::RootKind;
use crate::settings::LocalState;

/// What adding a folder does to the media folders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Placement {
    /// It is new: it joins the list.
    New,
    /// The library has it already, maybe by another path.
    Duplicate(PathBuf),
    /// It is inside this media folder, which already indexes it.
    Inside(PathBuf),
    /// It holds these media folders, which it replaces.
    Contains(Vec<PathBuf>),
}

/// A media folder of the settings and the path it resolves to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Existing {
    /// As the settings have it.
    pub path: PathBuf,
    /// Without links or `..`, as [`canonical`] gives it.
    pub canonical: PathBuf,
}

/// Where `canonical` (a folder's [`canonical`] path) goes among `existing`.
pub fn placement(canonical: &Path, existing: &[Existing]) -> Placement {
    if let Some(same) = existing.iter().find(|folder| folder.canonical == canonical) {
        return Placement::Duplicate(same.path.clone());
    }
    if let Some(outer) = existing.iter().find(|folder| canonical.starts_with(&folder.canonical)) {
        return Placement::Inside(outer.path.clone());
    }
    let inner: Vec<PathBuf> = existing
        .iter()
        .filter(|folder| folder.canonical.starts_with(canonical))
        .map(|folder| folder.path.clone())
        .collect();
    if inner.is_empty() { Placement::New } else { Placement::Contains(inner) }
}

/// Every media folder of `local`, video and music, with its canonical path. Reads the disk.
pub fn existing(local: &LocalState) -> Vec<Existing> {
    local
        .media_folders
        .iter()
        .chain(&local.music_folders)
        .map(|path| Existing { path: path.clone(), canonical: canonical(path) })
        .collect()
}

/// `path` without links or `..`; as it is when it can't be resolved (a drive that isn't
/// connected). Reads the disk.
pub fn canonical(path: &Path) -> PathBuf {
    let resolved = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    strip_verbatim(resolved)
}

/// Windows' canonical paths start with `\\?\`; the settings' paths don't.
fn strip_verbatim(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        _ => path,
    }
}

/// Adds `folder` to `local`'s `kind` folders as `placement` says: replacing the folders it
/// contains, and not at all when the library has it already. Answers whether it was added.
pub fn add(local: &mut LocalState, folder: &Path, kind: RootKind, placement: &Placement) -> bool {
    match placement {
        Placement::Duplicate(_) | Placement::Inside(_) => return false,
        Placement::Contains(inner) => {
            for path in inner {
                remove(local, path);
            }
        }
        Placement::New => {}
    }
    let folders = match kind {
        RootKind::Music => &mut local.music_folders,
        // The settings have no mixed folders: those are videos, as onboarding's is.
        RootKind::Video | RootKind::Mixed => &mut local.media_folders,
    };
    folders.push(folder.to_path_buf());
    true
}

/// Removes `folder` from `local`'s media folders, video or music. The library shows its empty
/// state once none is left.
pub fn remove(local: &mut LocalState, folder: &Path) {
    local.media_folders.retain(|path| path != folder);
    local.music_folders.retain(|path| path != folder);
}

/// Whether `folder` is a whole drive or the home folder, which takes long to index. Reads the
/// disk.
pub fn broad(folder: &Path) -> bool {
    folder.parent().is_none()
        || dirs::home_dir().is_some_and(|home| canonical(&home) == folder)
        || is_mount_point(folder)
}

/// Whether `folder` is where a drive is mounted (`/Volumes/Backup`, `/media/me/Disk`).
#[cfg(unix)]
fn is_mount_point(folder: &Path) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    let (Some(parent), Ok(own)) = (folder.parent(), fs::metadata(folder)) else { return false };
    fs::metadata(parent).is_ok_and(|parent| parent.dev() != own.dev())
}

/// Windows' drives are roots, which have no parent.
#[cfg(not(unix))]
fn is_mount_point(_: &Path) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn existing(paths: &[&str]) -> Vec<Existing> {
        paths.iter().map(|p| Existing { path: p.into(), canonical: p.into() }).collect()
    }

    #[test]
    fn a_folder_is_placed_by_its_canonical_path() {
        let folders = existing(&["/Users/me/Movies", "/Volumes/Music"]);
        assert_eq!(placement(Path::new("/Users/me/Shows"), &folders), Placement::New);
        assert_eq!(
            placement(Path::new("/Users/me/Movies"), &folders),
            Placement::Duplicate("/Users/me/Movies".into())
        );
        assert_eq!(
            placement(Path::new("/Users/me/Movies/Kids"), &folders),
            Placement::Inside("/Users/me/Movies".into())
        );
        assert_eq!(
            placement(Path::new("/Users/me"), &folders),
            Placement::Contains(vec!["/Users/me/Movies".into()])
        );
        // A sibling whose name starts the same is neither inside nor around it.
        assert_eq!(placement(Path::new("/Users/me/Movies 2"), &folders), Placement::New);
    }

    #[test]
    fn a_link_to_a_media_folder_is_the_same_folder() {
        let link = Existing { path: "~/Films".into(), canonical: "/Volumes/Films".into() };
        assert_eq!(
            placement(Path::new("/Volumes/Films"), &[link]),
            Placement::Duplicate("~/Films".into())
        );
    }

    #[test]
    fn adding_replaces_the_folders_inside_and_removing_empties() {
        let mut local = LocalState {
            media_folders: vec!["/m/Movies".into()],
            music_folders: vec!["/m/Music".into()],
            ..LocalState::default()
        };
        let inner = Placement::Contains(vec!["/m/Movies".into(), "/m/Music".into()]);
        assert!(add(&mut local, Path::new("/m"), RootKind::Video, &inner));
        assert_eq!(local.media_folders, [PathBuf::from("/m")]);
        assert_eq!(local.music_folders, Vec::<PathBuf>::new());
        assert!(!add(
            &mut local,
            Path::new("/m"),
            RootKind::Music,
            &Placement::Duplicate("/m".into())
        ));
        remove(&mut local, Path::new("/m"));
        assert_eq!(local.media_folders, Vec::<PathBuf>::new());
    }

    #[test]
    fn a_drive_root_is_broad() {
        assert!(broad(Path::new("/")));
    }
}
