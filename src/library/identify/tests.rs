//! Identifying real files into an in-memory library.

use std::ffi::OsStr;

use super::*;
use crate::db::files::Scanned;
use crate::db::folders;
use crate::db::roots::{self, RootKind};
use crate::db::{Database, now_ms};
use crate::library::path_key::Case;
use crate::opening::scratch::scratch_dir;

const LATER_MS: i64 = 60_000;

/// A library rooted at a scratch folder holding `files` (name, content), all unidentified.
fn library(files: &[(&str, &[u8])]) -> (Database, PathBuf) {
    let dir = scratch_dir();
    let mut database = Database::in_memory();
    let c = database.connection();
    roots::add(c, &dir, RootKind::Video, Case::Sensitive, 1).expect("root");
    let folder = folders::find(c, &dir, Case::Sensitive).expect("find").expect("root folder");
    for (name, content) in files {
        fs::write(dir.join(name), content).expect("write");
        let stamp = identity::stamp(&fs::metadata(dir.join(name)).expect("metadata"));
        let scanned = Scanned { name: OsStr::new(name), stamp, media: None, cloud: false };
        files::put(c, folder, &scanned, Case::Sensitive).expect("put");
    }
    (database, dir)
}

fn pending(database: &mut Database) -> Vec<Unidentified> {
    files::unidentified(database.connection(), FileId(0), 100).expect("query")
}

fn identify(database: &mut Database) -> Read {
    let batch = pending(database);
    let read = read(&batch, now_ms() + LATER_MS, &Cancel::default());
    link(database.connection(), &read.identified).expect("link");
    read
}

#[test]
fn files_still_being_written_wait() {
    let (mut database, _) = library(&[("a.mkv", b"movie")]);
    let batch = pending(&mut database);
    let read = read(&batch, now_ms(), &Cancel::default());
    assert_eq!(read.identified, []);
    assert!(read.unsettled);
    assert_eq!(identify(&mut database).identified.len(), 1);
    assert_eq!(pending(&mut database), []);
}

#[test]
fn copies_share_one_media_row() {
    let (mut database, dir) =
        library(&[("a.mkv", b"same"), ("b.mkv", b"same"), ("c.mkv", b"other")]);
    identify(&mut database);
    let c = database.connection();
    let key = |name: &str| identity::file_key(&dir.join(name), 4).expect("key");
    let id = media::find_identity(c, &key("a.mkv")).expect("query").expect("a has a row");
    let shared = files::of_media(c, id).expect("files");
    assert_eq!(shared.len(), 2);
    let other = identity::file_key(&dir.join("c.mkv"), 5).expect("key");
    assert!(media::find_identity(c, &other).expect("query").is_some_and(|row| row != id));
}

#[test]
fn a_moved_file_keeps_its_media_row_and_it_follows() {
    let (mut database, dir) = library(&[("old.mkv", b"movie")]);
    identify(&mut database);
    let c = database.connection();
    let before =
        files::in_folder(c, folders::find(c, &dir, Case::Sensitive).unwrap().unwrap()).unwrap();
    let id = before[0].media.expect("identified");
    // The scan saw it go and come back under a new name.
    files::remove(c, before[0].id).expect("remove");
    fs::rename(dir.join("old.mkv"), dir.join("new.mkv")).expect("rename");
    let folder = folders::find(c, &dir, Case::Sensitive).unwrap().unwrap();
    let stamp = identity::stamp(&fs::metadata(dir.join("new.mkv")).unwrap());
    let scanned = Scanned { name: OsStr::new("new.mkv"), stamp, media: None, cloud: false };
    files::put(c, folder, &scanned, Case::Sensitive).expect("put");
    identify(&mut database);
    let c = database.connection();
    let after = media::find_at(c, &stored_path::encode(&dir.join("new.mkv"))).expect("query");
    assert_eq!(after.map(|row| row.id), Some(id));
}

#[test]
fn a_file_changed_after_reading_is_left_for_later() {
    let (mut database, dir) = library(&[("a.mkv", b"one")]);
    let batch = pending(&mut database);
    let read = read(&batch, now_ms() + LATER_MS, &Cancel::default());
    // A scan saw it change before the result was written.
    let c = database.connection();
    let folder = folders::find(c, &dir, Case::Sensitive).unwrap().unwrap();
    let stamp = FileStamp { size: 99, modified_ms: Some(1) };
    let scanned = Scanned { name: OsStr::new("a.mkv"), stamp, media: None, cloud: false };
    files::put(c, folder, &scanned, Case::Sensitive).expect("put");
    link(c, &read.identified).expect("link");
    assert_eq!(pending(&mut database).len(), 1);
}

#[test]
fn online_only_and_missing_files_are_not_read() {
    let (mut database, dir) = library(&[("gone.mkv", b"x")]);
    fs::remove_file(dir.join("gone.mkv")).expect("remove");
    let read = identify(&mut database);
    assert!(read.identified.is_empty() && !read.unsettled);
    let c = database.connection();
    let folder = folders::find(c, &dir, Case::Sensitive).unwrap().unwrap();
    let stamp = FileStamp { size: 1, modified_ms: Some(1) };
    let cloud = Scanned { name: OsStr::new("cloud.mkv"), stamp, media: None, cloud: true };
    files::put(c, folder, &cloud, Case::Sensitive).expect("put");
    assert_eq!(pending(&mut database).len(), 1, "only the missing one is pending");
}
