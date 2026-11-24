//! Copying files and folders into a library folder (S4.8.2's Add File and Add Folder), which the
//! indexer then finds like any other change. Nothing is overwritten: a name already taken gets a
//! number, as file managers do ("clip 2.mkv"). A folder can't be copied into itself.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The most numbered names tried for one copy before giving up.
const MAX_NUMBER: u32 = 999;

/// What copying a batch did.
#[derive(Debug, Default)]
pub struct Copied {
    /// How many files were copied, counting those inside copied folders.
    pub files: usize,
    /// What couldn't be copied, and why.
    pub failed: Vec<(PathBuf, io::Error)>,
}

/// Copies each of `sources` (files or folders) into `folder`. Blocks on the disk.
pub fn copy_into(sources: &[PathBuf], folder: &Path) -> Copied {
    let mut copied = Copied::default();
    for source in sources {
        match copy_one(source, folder) {
            Ok(files) => copied.files += files,
            Err(error) => copied.failed.push((source.clone(), error)),
        }
    }
    copied
}

/// Copies `source` into `folder` under a free name, answering how many files that was.
fn copy_one(source: &Path, folder: &Path) -> io::Result<usize> {
    let name = source.file_name().ok_or_else(|| invalid(tr!("it has no name")))?;
    let target = free_name(folder, Path::new(name))?;
    if fs::metadata(source)?.is_dir() {
        if inside(folder, source) {
            return Err(invalid(tr!("a folder can't be copied into itself")));
        }
        copy_tree(source, &target)
    } else {
        fs::copy(source, &target).map(|_| 1)
    }
}

/// Copies the folder `from` to `to`, which doesn't exist yet, with everything in it.
fn copy_tree(from: &Path, to: &Path) -> io::Result<usize> {
    fs::create_dir(to)?;
    let mut files = 0;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        files += if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?
        } else {
            fs::copy(entry.path(), &target).map(|_| 1)?
        };
    }
    Ok(files)
}

/// `name` in `folder`, or the first of "name 2", "name 3", … (before the extension) that isn't
/// taken.
fn free_name(folder: &Path, name: &Path) -> io::Result<PathBuf> {
    let first = folder.join(name);
    if !exists(&first) {
        return Ok(first);
    }
    let stem = name.file_stem().unwrap_or(name.as_os_str()).to_string_lossy();
    let extension = name.extension().map(|e| format!(".{}", e.to_string_lossy()));
    (2..=MAX_NUMBER)
        .map(|n| folder.join(format!("{stem} {n}{}", extension.as_deref().unwrap_or(""))))
        .find(|candidate| !exists(candidate))
        .ok_or_else(|| io::Error::new(io::ErrorKind::AlreadyExists, tr!("every name is taken")))
}

/// Whether anything (even a broken link) is at `path`.
fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

/// Whether `path` is `folder` or below it, comparing real paths where they can be read.
fn inside(path: &Path, folder: &Path) -> bool {
    let real = |path: &Path| fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    real(path).starts_with(real(folder))
}

fn invalid(why: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, why)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opening::scratch::scratch_dir;

    fn write(path: &Path) {
        fs::create_dir_all(path.parent().expect("has a parent")).expect("parent");
        fs::write(path, b"x").expect("written");
    }

    #[test]
    fn a_taken_name_gets_a_number() {
        let (from, to) = (scratch_dir(), scratch_dir());
        write(&from.join("clip.mkv"));
        write(&to.join("clip.mkv"));
        write(&to.join("clip 2.mkv"));
        let copied = copy_into(&[from.join("clip.mkv")], &to);
        assert_eq!(copied.files, 1);
        assert!(copied.failed.is_empty());
        assert!(to.join("clip 3.mkv").is_file());
    }

    #[test]
    fn a_folder_is_copied_with_everything_in_it() {
        let (from, to) = (scratch_dir(), scratch_dir());
        write(&from.join("Show/S01/e1.mkv"));
        write(&from.join("Show/e0.mkv"));
        let copied = copy_into(&[from.join("Show")], &to);
        assert_eq!(copied.files, 2);
        assert!(to.join("Show/S01/e1.mkv").is_file());
    }

    #[test]
    fn a_folder_is_not_copied_into_itself() {
        let top = scratch_dir();
        write(&top.join("Show/e1.mkv"));
        let copied = copy_into(&[top.join("Show")], &top.join("Show"));
        assert_eq!(copied.files, 0);
        assert_eq!(copied.failed.len(), 1);
        assert!(!top.join("Show/Show").exists());
    }

    #[test]
    fn a_missing_source_fails_alone() {
        let (from, to) = (scratch_dir(), scratch_dir());
        write(&from.join("a.mkv"));
        let copied = copy_into(&[from.join("gone.mkv"), from.join("a.mkv")], &to);
        assert_eq!((copied.files, copied.failed.len()), (1, 1));
    }
}
