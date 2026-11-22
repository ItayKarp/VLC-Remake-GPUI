//! The thumbnail rows: what the thumbnailer reads about a media, its backlog, and eviction.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use super::*;
use crate::db::Database;
use crate::db::files::{self, Scanned};
use crate::db::media::{self, Entry, FileStamp};
use crate::db::metadata::{self, Extracted, Metadata};
use crate::db::roots::{self, RootKind};
use crate::library::path_key::Case;

/// A media with a file in `/v`, read as `kind`.
fn media_in_library(c: &mut Connection, identity: &str, kind: MediaKind) -> MediaId {
    roots::add(c, Path::new("/v"), RootKind::Video, Case::Sensitive, 1).expect("root");
    let folder = folders::find(c, Path::new("/v"), Case::Sensitive).expect("find").expect("root");
    let entry =
        Entry { identity: identity.into(), location: format!("/v/{identity}"), stamp: None };
    let id = media::insert(c, &entry).expect("insert");
    let name = format!("{identity}.mkv");
    let stamp = FileStamp { size: 1, modified_ms: Some(1) };
    let file = Scanned { name: OsStr::new(&name), stamp, media: Some(id), cloud: false };
    files::put(c, folder, &file, Case::Sensitive).expect("file");
    let extracted = Extracted {
        metadata: Metadata { kind, ..Metadata::default() },
        duration: Some(Duration::from_secs(60)),
        artwork: None,
    };
    metadata::set_extracted(c, id, &extracted, 5).expect("read");
    id
}

fn image(variant: Variant, key: &str, bytes: u64) -> Cached {
    Cached { variant, cache_key: key.into(), bytes }
}

#[test]
fn a_subject_has_its_file_length_and_kind() {
    let mut database = Database::in_memory();
    let c = database.connection();
    let id = media_in_library(c, "film", MediaKind::Video);
    let subject = subject(c, id).expect("query").expect("exists");
    assert_eq!(subject.path, Some(PathBuf::from("/v/film.mkv")));
    assert_eq!((subject.kind, subject.duration), (MediaKind::Video, Some(Duration::from_secs(60))));
    assert!(!subject.gave_up);
    assert_eq!(super::subject(c, MediaId(99)).expect("query"), None);
}

#[test]
fn the_backlog_is_videos_without_thumbnails_or_a_mark() {
    let mut database = Database::in_memory();
    let c = database.connection();
    let (film, song) = (
        media_in_library(c, "film", MediaKind::Video),
        media_in_library(c, "song", MediaKind::Audio),
    );
    let other = media_in_library(c, "other", MediaKind::Video);
    assert_eq!(backlog(c, 10).expect("backlog"), [other, film]);
    give_up(c, other, 7).expect("give up");
    assert!(subject(c, other).expect("query").expect("exists").gave_up);
    record(c, film, &[image(VARIANTS[0], "k", 10)], 8).expect("record");
    assert_eq!(backlog(c, 10).expect("backlog"), []);
    assert_eq!(cached(c, film).expect("cached"), [image(VARIANTS[0], "k", 10)]);
    // Reading the file again (Resync) tries again.
    metadata::reprobe(c, other).expect("reprobe");
    assert!(!subject(c, other).expect("query").expect("exists").gave_up);
    let _ = song;
}

#[test]
fn shared_images_count_once_and_the_least_recent_go_first() {
    let mut database = Database::in_memory();
    let c = database.connection();
    let (a, b, d) = (
        media_in_library(c, "a", MediaKind::Audio),
        media_in_library(c, "b", MediaKind::Audio),
        media_in_library(c, "d", MediaKind::Video),
    );
    record(c, a, &[image(VARIANTS[0], "cover", 100)], 1).expect("record");
    record(c, b, &[image(VARIANTS[0], "cover", 100)], 2).expect("record");
    record(c, d, &[image(VARIANTS[0], "frame", 50)], 3).expect("record");
    assert_eq!(total_bytes(c).expect("total"), 150);
    touch(c, a, 9).expect("touch");
    assert_eq!(least_recent(c, 1).expect("lru"), [("frame".to_owned(), 50)]);
    evict(c, "cover").expect("evict");
    assert_eq!((cached(c, a).expect("a"), cached(c, b).expect("b")), (vec![], vec![]));
    forget(c, d).expect("forget");
    assert_eq!(total_bytes(c).expect("total"), 0);
}
