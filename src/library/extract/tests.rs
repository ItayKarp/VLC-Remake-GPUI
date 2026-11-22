//! Metadata extraction on an in-memory library, with a reader that answers from a table.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use image::{ImageFormat, RgbImage};

use super::*;
use crate::db::files::{self, Scanned};
use crate::db::folders;
use crate::db::media::{self, Entry, FileStamp};
use crate::db::metadata::Metadata;
use crate::db::roots::{self, RootKind};
use crate::library::identify::{self, Identified};
use crate::library::indexer::store::InPlace;
use crate::library::path_key::Case;
use crate::opening::scratch::scratch_dir;

const STAMP: FileStamp = FileStamp { size: 10, modified_ms: Some(1) };

/// Answers each file name with its probe, counting reads.
struct Table {
    answers: HashMap<PathBuf, Probe>,
    reads: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl TagReader for Table {
    fn read(&mut self, path: &Path, _: Lookup) -> Probe {
        self.reads.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.answers.get(path).cloned().unwrap_or(Probe::Unreadable)
    }
}

/// A library holding `names` in `dir`, each its own media.
fn library(dir: &Path, names: &[&str]) -> (InPlace, Vec<MediaId>) {
    let store = InPlace::new();
    let mut ids = Vec::new();
    {
        let mut database = store.0.lock().expect("not poisoned");
        let c = database.connection();
        roots::add(c, dir, RootKind::Music, Case::Sensitive, 1).expect("root");
        let folder = folders::find(c, dir, Case::Sensitive).expect("find").expect("added");
        for name in names {
            let entry = Entry { identity: (*name).into(), location: (*name).into(), stamp: None };
            let id = media::insert(c, &entry).expect("media");
            let file =
                Scanned { name: OsStr::new(name), stamp: STAMP, media: Some(id), cloud: false };
            files::put(c, folder, &file, Case::Sensitive).expect("file");
            ids.push(id);
        }
    }
    (store, ids)
}

fn song(title: &str, artwork: Option<PathBuf>) -> Probe {
    let metadata =
        Metadata { kind: MediaKind::Audio, title: Some(title.into()), ..Metadata::default() };
    Probe::Read(Box::new(Probed { metadata, duration: Some(Duration::from_secs(200)), artwork }))
}

/// Writes a small picture of one `shade`; pictures of different shades are different covers.
fn picture(path: &Path, shade: u8) {
    RgbImage::from_pixel(8, 8, image::Rgb([shade, 2, 3]))
        .save_with_format(path, ImageFormat::Png)
        .expect("write a picture");
}

fn extractor(
    dir: &Path,
    answers: Vec<(PathBuf, Probe)>,
) -> (Extractor, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    let reads = std::sync::Arc::default();
    let reader =
        Table { answers: answers.into_iter().collect(), reads: std::sync::Arc::clone(&reads) };
    (Extractor::new(Box::new(reader), Some(ArtworkCache::new(dir.join(".cache")))), reads)
}

fn run_all(extractor: &mut Extractor, store: &InPlace) {
    let cancel = Cancel::default();
    while extractor.run(store, &cancel) == Pass::More {}
}

#[test]
fn tags_length_and_covers_are_written_once() {
    let dir = scratch_dir();
    let (store, ids) = library(&dir, &["a.mp3", "b.mp3", "broken.mp3"]);
    let embedded_dir = scratch_dir();
    picture(&embedded_dir.join("art.png"), 1);
    picture(&dir.join("cover.jpg"), 2);
    let answers = vec![
        (dir.join("a.mp3"), song("A", Some(embedded_dir.join("art.png")))),
        (dir.join("b.mp3"), song("B", None)),
    ];
    let (mut extractor, reads) = extractor(&dir, answers);
    run_all(&mut extractor, &store);
    let mut database = store.0.lock().expect("not poisoned");
    let c = database.connection();
    let a = metadata::get(c, ids[0]).expect("a");
    assert_eq!((a.kind, a.title.as_deref()), (MediaKind::Audio, Some("A")));
    assert_eq!(media::saved(c, ids[0]).expect("saved").duration, Some(Duration::from_secs(200)));
    let source = |id| artwork::of_media(c, id).expect("artwork").map(|art| art.source);
    assert_eq!(source(ids[0]), Some(ArtworkSource::Embedded));
    assert_eq!(source(ids[1]), Some(ArtworkSource::Folder));
    assert_eq!(source(ids[2]), None);
    assert_eq!(metadata::unprobed(c, MediaId(0), 10).expect("unprobed"), []);
    drop(database);
    run_all(&mut extractor, &store);
    assert_eq!(reads.load(std::sync::atomic::Ordering::Relaxed), 3, "each media is read once");
}

#[test]
fn a_file_identified_again_is_read_again() {
    let dir = scratch_dir();
    let (store, ids) = library(&dir, &["a.mp3"]);
    let (mut extractor, reads) = extractor(&dir, vec![(dir.join("a.mp3"), song("A", None))]);
    run_all(&mut extractor, &store);
    let file = {
        let mut database = store.0.lock().expect("not poisoned");
        files::of_media(database.connection(), ids[0]).expect("files").remove(0)
    };
    let again = Identified { id: file.id, path: file.path, stamp: STAMP, key: "a.mp3".into() };
    identify::link(store.0.lock().expect("not poisoned").connection(), &[again]).expect("link");
    run_all(&mut extractor, &store);
    assert_eq!(reads.load(std::sync::atomic::Ordering::Relaxed), 2);
}

#[test]
fn nothing_is_marked_read_without_a_reader() {
    let dir = scratch_dir();
    let (store, _) = library(&dir, &["a.mp3"]);
    let mut extractor = Extractor::new(Box::new(crate::engine::tags::NoTags), None);
    assert_eq!(extractor.run(&store, &Cancel::default()), Pass::Done);
    let mut database = store.0.lock().expect("not poisoned");
    assert_eq!(metadata::unprobed(database.connection(), MediaId(0), 10).expect("q").len(), 1);
}

#[test]
fn videos_dont_take_the_folders_cover() {
    let dir = scratch_dir();
    picture(&dir.join("folder.jpg"), 3);
    let movie = dir.join("movie.mkv");
    assert_eq!(choose_artwork(None, &movie, MediaKind::Video, Lookup::Local), None);
    assert_eq!(
        choose_artwork(Some(&dir.join("folder.jpg")), &movie, MediaKind::Audio, Lookup::Local),
        Some((dir.join("folder.jpg"), ArtworkSource::Folder))
    );
}

#[test]
fn from_the_file_only_leaves_the_folders_cover_alone() {
    let dir = scratch_dir();
    picture(&dir.join("folder.jpg"), 3);
    let song = dir.join("song.mp3");
    assert_eq!(choose_artwork(None, &song, MediaKind::Audio, Lookup::File), None);
    let inside = std::env::temp_dir().join("embedded.jpg");
    assert_eq!(
        choose_artwork(Some(&inside), &song, MediaKind::Audio, Lookup::File),
        Some((inside, ArtworkSource::Embedded))
    );
}
