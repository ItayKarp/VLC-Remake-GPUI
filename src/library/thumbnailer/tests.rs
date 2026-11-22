//! The thumbnailer thread with a fake grabber and an in-memory library.

use std::ffi::OsStr;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use image::{ImageFormat, RgbImage};

use super::make::Maker;
use super::{Answer, Command, spawn};
use crate::db::artwork::{self, ArtworkSource};
use crate::db::files::{self, Scanned};
use crate::db::folders;
use crate::db::media::{self, Entry, FileStamp, MediaId};
use crate::db::metadata::{self, Extracted, MediaKind, Metadata};
use crate::db::roots::{self, RootKind};
use crate::db::thumbnails::{self, SizeClass};
use crate::engine::thumbnail::{Frame, FrameGrabber, Grab};
use crate::history::DatabaseWork;
use crate::library::artwork_cache::ArtworkCache;
use crate::library::indexer::store::{InPlace, Store, call};
use crate::library::path_key::Case;
use crate::library::thumbnail_cache::ThumbnailCache;
use crate::opening::Cancel;
use crate::opening::scratch::scratch_dir;

/// How long a test waits for an answer.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(10);

/// The test's view of the database, shared with the thread.
#[derive(Clone)]
struct Shared(Arc<InPlace>);

impl Store for Shared {
    fn run(&self, work: DatabaseWork) -> bool {
        self.0.run(work)
    }
}

/// Answers every grab with `frame`, counting them.
struct Fake {
    frame: Option<Frame>,
    grabs: Arc<AtomicUsize>,
}

impl FrameGrabber for Fake {
    fn grab(&mut self, _: &Path, _: Duration) -> Grab {
        self.grabs.fetch_add(1, Ordering::Relaxed);
        self.frame.clone().map_or(Grab::Nothing, Grab::Frame)
    }
}

fn striped() -> Frame {
    let bgra = (0..64 * 36).flat_map(|i| if i % 2 == 0 { [0; 4] } else { [250; 4] }).collect();
    Frame { width: 64, height: 36, bgra }
}

/// Adds a media read as `kind`, with a file in `/v`.
fn add(store: &Shared, identity: &'static str, kind: MediaKind, cover: Option<String>) -> MediaId {
    call(store, move |c| {
        roots::add(c, Path::new("/v"), RootKind::Video, Case::Sensitive, 1)?;
        let folder = folders::find(c, Path::new("/v"), Case::Sensitive)?.expect("the root");
        let entry = Entry { identity: identity.into(), location: identity.into(), stamp: None };
        let id = media::insert(c, &entry)?;
        let stamp = FileStamp { size: 1, modified_ms: Some(1) };
        let name = OsStr::new(identity);
        files::put(
            c,
            folder,
            &Scanned { name, stamp, media: Some(id), cloud: false },
            Case::Sensitive,
        )?;
        let artwork = match cover {
            Some(key) => Some(artwork::ensure(c, &key, ArtworkSource::Embedded)?),
            None => None,
        };
        let metadata = Metadata { kind, ..Metadata::default() };
        let duration = Some(Duration::from_secs(100));
        metadata::set_extracted(c, id, &Extracted { metadata, duration, artwork }, 1)?;
        Ok(id)
    })
    .expect("added")
}

struct Running {
    store: Shared,
    commands: mpsc::Sender<Command>,
    answers: mpsc::Receiver<Answer>,
    grabs: Arc<AtomicUsize>,
    cancel: Cancel,
}

impl Running {
    fn answer(&self) -> Answer {
        self.answers.recv_timeout(ANSWER_TIMEOUT).expect("an answer")
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

fn start(store: Shared, frame: Option<Frame>, covers: Option<ArtworkCache>) -> Running {
    let grabs = Arc::new(AtomicUsize::new(0));
    let grabber = Box::new(Fake { frame, grabs: grabs.clone() });
    let maker = Maker::new(grabber, ThumbnailCache::new(scratch_dir().join("thumbs")), covers);
    let (reply, answers) = mpsc::channel();
    let report = Box::new(move |answer| drop(reply.send(answer)));
    let cancel = Cancel::default();
    let commands = spawn(Box::new(store.clone()), maker, report, cancel.clone());
    Running { store, commands, answers, grabs, cancel }
}

#[test]
fn the_backlog_fills_the_cache_and_views_are_answered_from_it() {
    let store = Shared(Arc::new(InPlace::new()));
    let film = add(&store, "film", MediaKind::Video, None);
    let running = start(store, Some(striped()), None);
    let made = running.answer();
    assert_eq!(made.media, film);
    let thumbnail = made.thumbnail.expect("a picture");
    let grid = thumbnail.path(SizeClass::Grid, 2).expect("the 2x grid picture");
    assert!(grid.is_file());
    // A usable first frame is kept: one grab.
    assert_eq!(running.grabs.load(Ordering::Relaxed), 1);
    running.commands.send(Command::Show(vec![film])).expect("send");
    assert_eq!(running.answer().thumbnail, Some(thumbnail));
    assert_eq!(running.grabs.load(Ordering::Relaxed), 1);
}

#[test]
fn a_video_without_a_picture_is_marked_and_not_tried_again() {
    let store = Shared(Arc::new(InPlace::new()));
    let film = add(&store, "broken", MediaKind::Video, None);
    let running = start(store, None, None);
    assert_eq!(running.answer(), Answer { media: film, thumbnail: None });
    let tries = running.grabs.load(Ordering::Relaxed);
    assert!(tries > 1, "every time is tried");
    running.commands.send(Command::Show(vec![film])).expect("send");
    assert_eq!(running.answer(), Answer { media: film, thumbnail: None });
    assert_eq!(running.grabs.load(Ordering::Relaxed), tries);
    let subject = call(&running.store, move |c| thumbnails::subject(c, film)).flatten();
    assert!(subject.expect("the media").gave_up);
}

#[test]
fn a_song_on_screen_gets_its_cover_without_grabbing() {
    let dir = scratch_dir();
    let covers = ArtworkCache::new(dir.join("art"));
    let source = dir.join("cover.png");
    RgbImage::from_pixel(300, 300, image::Rgb([20, 200, 90]))
        .save_with_format(&source, ImageFormat::Png)
        .expect("a cover");
    let key = covers.store(&source).expect("cached");
    let store = Shared(Arc::new(InPlace::new()));
    let song = add(&store, "song", MediaKind::Audio, Some(key));
    let running = start(store, Some(striped()), Some(covers));
    running.commands.send(Command::Show(vec![song])).expect("send");
    let answer = running.answer();
    assert_eq!(answer.media, song);
    let list = answer.thumbnail.expect("the cover").path(SizeClass::List, 1).map(Path::to_path_buf);
    let list = image::open(list.expect("a list picture")).expect("a JPEG");
    assert_eq!((list.width(), list.height()), (32, 32));
    assert_eq!(running.grabs.load(Ordering::Relaxed), 0);
}
