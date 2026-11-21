//! Writing listings into an in-memory library.

use std::ffi::OsString;

use super::*;
use crate::db::Database;
use crate::db::media::{self, Entry, FileStamp, MediaId};
use crate::db::roots::{self, RootKind};

const CASE: Case = Case::Sensitive;

fn stamp(size: u64) -> FileStamp {
    FileStamp { size, modified_ms: Some(1) }
}

fn listing(dir: &str, files: &[(&str, u64)], subdirs: &[&str]) -> Listing {
    Listing {
        dir: dir.into(),
        files: files
            .iter()
            .map(|(name, size)| Found { name: (*name).into(), stamp: stamp(*size), cloud: false })
            .collect(),
        subdirs: subdirs.iter().map(OsString::from).collect(),
    }
}

fn library() -> Database {
    let mut database = Database::in_memory();
    roots::add(database.connection(), Path::new("/v"), RootKind::Video, CASE, 1).expect("root");
    database
}

fn folder(connection: &Connection, path: &str) -> Option<FolderId> {
    folders::find(connection, Path::new(path), CASE).expect("find")
}

fn names(connection: &Connection, path: &str) -> Vec<(String, Option<MediaId>)> {
    let folder = folder(connection, path).expect("folder");
    let files = files::in_folder(connection, folder).expect("files");
    files
        .into_iter()
        .map(|f| (f.path.file_name().expect("name").to_string_lossy().into(), f.media))
        .collect()
}

#[test]
fn a_first_scan_adds_folders_and_files() {
    let mut database = library();
    let c = database.connection();
    let top = listing("/v", &[("a.mkv", 1)], &["tv"]);
    let applied =
        apply_all(c, &[top, listing("/v/tv", &[("e1.mkv", 2)], &[])], CASE, Rescan::Changed)
            .expect("apply");
    assert_eq!(applied, Applied { changed: 2, removed: 0, new_subdirs: vec!["/v/tv".into()] });
    assert_eq!(names(c, "/v"), [("a.mkv".to_owned(), None)]);
    assert_eq!(names(c, "/v/tv"), [("e1.mkv".to_owned(), None)]);
    let v = folder(c, "/v").expect("root folder");
    let tv = folders::children(c, v).expect("children");
    assert_eq!(tv.iter().map(|f| f.path.clone()).collect::<Vec<_>>(), [PathBuf::from("/v/tv")]);
}

#[test]
fn unchanged_files_keep_their_media_and_changed_ones_lose_it() {
    let mut database = library();
    let c = database.connection();
    apply(c, &listing("/v", &[("a.mkv", 1), ("b.mkv", 1)], &[]), CASE, Rescan::Changed)
        .expect("apply");
    let id =
        media::insert(c, &Entry { identity: "k".into(), location: "/v/a.mkv".into(), stamp: None })
            .expect("media");
    let v = folder(c, "/v").expect("folder");
    for file in files::in_folder(c, v).expect("files") {
        files::set_media(c, file.id, id, file.stamp).expect("link");
    }
    let applied =
        apply(c, &listing("/v", &[("a.mkv", 1), ("b.mkv", 5)], &[]), CASE, Rescan::Changed)
            .expect("apply");
    assert_eq!(applied.changed, 1);
    assert_eq!(names(c, "/v"), [("a.mkv".to_owned(), Some(id)), ("b.mkv".to_owned(), None)]);
}

#[test]
fn files_and_folders_that_are_gone_are_removed() {
    let mut database = library();
    let c = database.connection();
    let first = [listing("/v", &[("a.mkv", 1)], &["old"]), listing("/v/old", &[("x.mkv", 1)], &[])];
    apply_all(c, &first, CASE, Rescan::Changed).expect("apply");
    let applied = apply(c, &listing("/v", &[], &[]), CASE, Rescan::Changed).expect("apply");
    assert_eq!((applied.changed, applied.removed), (0, 1));
    assert_eq!(names(c, "/v"), []);
    assert_eq!(folder(c, "/v/old"), None);
    // The root's own folder is never removed by a listing of its parent.
    assert!(folder(c, "/v").is_some());
}

#[test]
fn a_resync_identifies_every_file_again() {
    let mut database = library();
    let c = database.connection();
    let first = listing("/v", &[("a.mkv", 1)], &[]);
    apply(c, &first, CASE, Rescan::Changed).expect("apply");
    assert_eq!(apply(c, &first, CASE, Rescan::Changed).expect("apply").changed, 0);
    assert_eq!(apply(c, &first, CASE, Rescan::Everything).expect("apply").changed, 1);
}

#[test]
fn only_new_subfolders_are_named() {
    let mut database = library();
    let c = database.connection();
    apply_all(c, &[listing("/v", &[], &["a"]), listing("/v/a", &[], &[])], CASE, Rescan::Changed)
        .expect("apply");
    let applied = apply(c, &listing("/v", &[], &["a", "b"]), CASE, Rescan::Changed).expect("apply");
    assert_eq!(applied.new_subdirs, [PathBuf::from("/v/b")]);
}

#[test]
fn names_compare_as_the_file_system_does() {
    let mut database = Database::in_memory();
    let c = database.connection();
    roots::add(c, Path::new("/v"), RootKind::Video, Case::Insensitive, 1).expect("root");
    apply(c, &listing("/v", &[("Movie.MKV", 1)], &[]), Case::Insensitive, Rescan::Changed)
        .expect("apply");
    let applied =
        apply(c, &listing("/v", &[("movie.mkv", 1)], &[]), Case::Insensitive, Rescan::Changed)
            .expect("apply");
    assert_eq!((applied.changed, applied.removed), (0, 0));
}
