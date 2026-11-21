//! Walking real folders in the temporary folder.

use std::collections::BTreeMap;
use std::sync::mpsc;

use super::*;
use crate::opening::scratch::scratch_dir;

const TREE: Options = Options { depth: Depth::Tree, show_hidden: false };

/// Each listed folder (relative to `top`) and its media file names, sorted; and the unreadable
/// folders.
fn run(top: &Path, options: Options) -> (BTreeMap<String, Vec<String>>, Vec<PathBuf>) {
    let (sink, events) = mpsc::sync_channel(1);
    let counters = Counters::default();
    let cancel = Cancel::default();
    let (mut listed, mut unreadable) = (BTreeMap::new(), Vec::new());
    thread::scope(|scope| {
        let (cancel, counters) = (&cancel, &counters);
        scope.spawn(move || walk(top, options, cancel, counters, &sink));
        for event in events {
            match event {
                Walked::Listed(listing) => {
                    let dir = listing.dir.strip_prefix(top).expect("below top");
                    let mut names: Vec<String> =
                        listing.files.iter().map(|f| f.name.to_string_lossy().into()).collect();
                    names.sort();
                    listed.insert(dir.to_string_lossy().replace('\\', "/"), names);
                }
                Walked::Unreadable(dir, _) => unreadable.push(dir),
            }
        }
    });
    assert_eq!(counters.found.load(Ordering::Relaxed), counters.listed.load(Ordering::Relaxed));
    (listed, unreadable)
}

fn write(top: &Path, files: &[&str]) {
    for file in files {
        let path = top.join(file);
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(path, b"x").expect("write");
    }
}

#[test]
fn lists_every_folder_with_its_media() {
    let top = scratch_dir();
    write(&top, &["a.mkv", "b.txt", "Season 1/e1.mp4", "Season 1/Extras/x.mp3", ".hidden/c.mkv"]);
    write(&top, &[".d.mkv", "Empty/readme.md"]);
    let (listed, unreadable) = run(&top, TREE);
    let expected: BTreeMap<String, Vec<String>> = [
        ("", vec!["a.mkv"]),
        ("Empty", vec![]),
        ("Season 1", vec!["e1.mp4"]),
        ("Season 1/Extras", vec!["x.mp3"]),
    ]
    .into_iter()
    .map(|(dir, names)| (dir.to_owned(), names.into_iter().map(String::from).collect()))
    .collect();
    assert_eq!(listed, expected);
    assert_eq!(unreadable, Vec::<PathBuf>::new());
}

#[test]
fn hidden_files_are_listed_when_asked_for() {
    let top = scratch_dir();
    write(&top, &[".d.mkv", ".hidden/c.mkv"]);
    let (listed, _) = run(&top, Options { show_hidden: true, ..TREE });
    assert_eq!(listed[""], [".d.mkv"]);
    assert_eq!(listed[".hidden"], ["c.mkv"]);
}

#[test]
fn a_shallow_walk_lists_one_folder_and_names_its_subfolders() {
    let top = scratch_dir();
    write(&top, &["a.mkv", "sub/b.mkv"]);
    let (sink, events) = mpsc::sync_channel(4);
    let options = Options { depth: Depth::Shallow, ..TREE };
    walk(&top, options, &Cancel::default(), &Counters::default(), &sink);
    drop(sink);
    let listings: Vec<Walked> = events.into_iter().collect();
    let [Walked::Listed(listing)] = &listings[..] else { panic!("one listing: {listings:?}") };
    assert_eq!(listing.subdirs, [OsString::from("sub")]);
    assert_eq!(listing.files.len(), 1);
    assert_eq!(listing.files[0].stamp.size, 1);
}

#[test]
fn a_missing_folder_is_reported() {
    let top = scratch_dir().join("gone");
    let (listed, unreadable) = run(&top, TREE);
    assert_eq!(listed, BTreeMap::new());
    assert_eq!(unreadable, [top]);
}

#[test]
fn a_cancelled_walk_lists_nothing() {
    let top = scratch_dir();
    write(&top, &["a.mkv"]);
    let (sink, events) = mpsc::sync_channel(4);
    let cancel = Cancel::default();
    cancel.cancel();
    walk(&top, TREE, &cancel, &Counters::default(), &sink);
    drop(sink);
    assert_eq!(events.into_iter().count(), 0);
}

#[cfg(unix)]
#[test]
fn symlink_loops_are_listed_once() {
    let top = scratch_dir();
    write(&top, &["sub/a.mkv"]);
    std::os::unix::fs::symlink(&top, top.join("sub/back")).expect("link");
    std::os::unix::fs::symlink(top.join("sub"), top.join("again")).expect("link");
    let (listed, _) = run(&top, TREE);
    assert_eq!(listed.len(), 2, "{listed:?}");
    assert_eq!(listed.values().flatten().count(), 1);
}

#[cfg(unix)]
#[test]
fn a_folder_without_permission_is_reported_and_skipped() {
    use std::os::unix::fs::PermissionsExt as _;
    let top = scratch_dir();
    write(&top, &["a.mkv", "locked/b.mkv"]);
    let locked = top.join("locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("chmod");
    let (listed, unreadable) = run(&top, TREE);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).expect("chmod");
    // Root can read anything; then there's nothing to check.
    if listed.contains_key("locked") {
        return;
    }
    assert_eq!(listed.keys().collect::<Vec<_>>(), [""]);
    assert_eq!(unreadable, [locked]);
}
