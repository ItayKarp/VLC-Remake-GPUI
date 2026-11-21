//! What to rescan when the watcher says a path changed (S4.2.4): the media folder it is in, and
//! the smallest folder whose rescan catches the change.
//!
//! - A changed folder is walked with everything below it (it may be new, or moved in).
//! - A changed file only needs its folder listed again; that listing also notices subfolders that
//!   came or went.
//! - A folder the library doesn't know yet is walked from its nearest known ancestor, so it
//!   hangs below its parent.

use std::path::{Path, PathBuf};

use super::store::{Store, call};
use crate::db::folders;
use crate::db::roots::Root;
use crate::library::walk::Depth;

/// A media folder, with where the watcher may report it: its path with links resolved
/// (`/private/var` for `/var` on macOS).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Watched {
    /// The media folder.
    pub root: Root,
    /// Its path with links resolved, when that differs.
    pub resolved: Option<PathBuf>,
}

impl Watched {
    /// `root`, with its links resolved.
    pub fn new(root: Root) -> Self {
        let resolved = std::fs::canonicalize(&root.path).ok().filter(|path| *path != root.path);
        Self { root, resolved }
    }

    /// `path` as a path below this media folder, if it is below it by either name.
    fn relocate(&self, path: &Path) -> Option<PathBuf> {
        if path.starts_with(&self.root.path) {
            return Some(path.to_path_buf());
        }
        let rest = path.strip_prefix(self.resolved.as_ref()?).ok()?;
        Some(self.root.path.join(rest))
    }
}

/// A rescan the watcher asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    /// Which media folder (index into the list the target was found in).
    pub root: usize,
    /// The folder to walk from.
    pub dir: PathBuf,
    /// How deep.
    pub depth: Depth,
}

/// The rescan that catches a change at `path`, if it is in one of `roots` (the innermost one).
pub fn target(store: &dyn Store, roots: &[Watched], path: &Path) -> Option<Target> {
    let (index, path) = roots
        .iter()
        .enumerate()
        .filter_map(|(index, watched)| Some((index, watched.relocate(path)?)))
        .max_by_key(|(index, _)| roots[*index].root.path.components().count())?;
    let root = &roots[index].root;
    let (dir, depth) = if path.is_dir() {
        (path, Depth::Tree)
    } else {
        (path.parent().unwrap_or(&root.path).to_path_buf(), Depth::Shallow)
    };
    // The media folder itself changed (renamed, deleted, or back): check it whole.
    if !dir.starts_with(&root.path) {
        return Some(Target { root: index, dir: root.path.clone(), depth: Depth::Tree });
    }
    if dir == root.path {
        return Some(Target { root: index, dir, depth });
    }
    let (top, case) = (root.path.clone(), root.case);
    let (dir, depth) = call(store, move |c| {
        let (mut dir, mut depth) = (dir, depth);
        while dir != top {
            let Some(parent) = dir.parent() else { break };
            if folders::find(c, parent, case)?.is_some() {
                break;
            }
            dir = parent.to_path_buf();
            depth = Depth::Tree;
        }
        Ok((dir, depth))
    })?;
    Some(Target { root: index, dir, depth })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::db::roots::{self, RootKind};
    use crate::library::indexer::store::InPlace;
    use crate::library::path_key::Case;
    use crate::opening::scratch::scratch_dir;

    fn setup() -> (InPlace, Vec<Watched>, PathBuf) {
        let top = scratch_dir();
        fs::create_dir_all(top.join("known/new")).expect("mkdir");
        let store = InPlace::new();
        let mut database = store.0.lock().expect("lock");
        let c = database.connection();
        roots::add(c, &top, RootKind::Video, Case::Sensitive, 1).expect("root");
        let folder = folders::find(c, &top, Case::Sensitive).expect("find");
        folders::ensure(c, &top.join("known"), folder, Case::Sensitive).expect("folder");
        let roots = roots::list(c).expect("roots").into_iter().map(Watched::new).collect();
        drop(database);
        (store, roots, top)
    }

    #[test]
    fn a_changed_file_rescans_its_folder() {
        let (store, roots, top) = setup();
        let found = target(&store, &roots, &top.join("known/a.mkv"));
        let expected = Target { root: 0, dir: top.join("known"), depth: Depth::Shallow };
        assert_eq!(found, Some(expected));
    }

    #[test]
    fn a_new_folder_is_walked_whole() {
        let (store, roots, top) = setup();
        let found = target(&store, &roots, &top.join("known/new"));
        assert_eq!(found, Some(Target { root: 0, dir: top.join("known/new"), depth: Depth::Tree }));
        // A file deep in folders the library doesn't know walks from the first unknown one.
        let deep = target(&store, &roots, &top.join("unknown/deeper/a.mkv"));
        assert_eq!(deep, Some(Target { root: 0, dir: top.join("unknown"), depth: Depth::Tree }));
    }

    #[test]
    fn changes_at_the_top_or_outside_are_handled() {
        let (store, roots, top) = setup();
        let found = target(&store, &roots, &top.join("a.mkv")).expect("in the root");
        assert_eq!((found.dir, found.depth), (top, Depth::Shallow));
        assert_eq!(target(&store, &roots, Path::new("/elsewhere/a.mkv")), None);
    }

    #[test]
    fn resolved_paths_count_as_the_root() {
        let (store, mut roots, top) = setup();
        roots[0].resolved = Some(PathBuf::from("/resolved"));
        let found = target(&store, &roots, Path::new("/resolved/known/a.mkv"));
        assert_eq!(found.map(|t| t.dir), Some(top.join("known")));
    }
}
