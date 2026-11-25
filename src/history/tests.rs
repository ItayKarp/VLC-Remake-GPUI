//! Recognizing media across moves, copies and edits (S1.7.2), and sessions saving through the
//! history thread's jobs (S1.7.3).

pub(crate) mod support;

use std::fs::{self, File};
use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, SystemTime};

use futures::channel::oneshot;

use super::History;
use super::resolve::{ResolveError, identity_of, resolve};
use super::worker::{Job, Worker};
use crate::app::media::Media;
use crate::db::Database;
use crate::db::media::{self, MediaId, Progress, Saved};
use crate::opening::scratch::scratch_dir;

fn write(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).expect("the scratch folder is writable");
}

fn id_of(database: &mut Database, path: &Path) -> MediaId {
    let media = Media::File(path.to_path_buf());
    match resolve(database.connection(), &media, 1_000) {
        Ok(id) => id,
        Err(error) => panic!("couldn't resolve {}: {error:?}", path.display()),
    }
}

#[test]
fn a_moved_file_keeps_its_history() {
    let dir = scratch_dir();
    let mut database = Database::in_memory();
    let (before, after) = (dir.join("a.mkv"), dir.join("renamed.mkv"));
    write(&before, b"episode one");
    let id = id_of(&mut database, &before);
    assert_eq!(id_of(&mut database, &before), id, "the same file is found again");
    fs::rename(&before, &after).expect("rename");
    assert_eq!(id_of(&mut database, &after), id);
    let location = media::find_at(database.connection(), &after.to_string_lossy());
    assert!(location.expect("query").is_some_and(|row| row.id == id));
}

#[test]
fn copies_are_the_same_media() {
    let dir = scratch_dir();
    let mut database = Database::in_memory();
    write(&dir.join("a.mkv"), b"film");
    write(&dir.join("b.mkv"), b"film");
    let id = id_of(&mut database, &dir.join("a.mkv"));
    assert_eq!(id_of(&mut database, &dir.join("b.mkv")), id);
    assert_eq!(id_of(&mut database, &dir.join("a.mkv")), id);
}

#[test]
fn a_file_changed_in_place_is_new_media_and_the_old_history_is_orphaned() {
    let dir = scratch_dir();
    let mut database = Database::in_memory();
    let path = dir.join("a.mkv");
    write(&path, b"first cut");
    let old = id_of(&mut database, &path);
    write(&path, b"director's cut, longer");
    let new = id_of(&mut database, &path);
    assert_ne!(new, old);
    // Orphaned rows are purged once old enough.
    assert_eq!(media::purge_orphans(database.connection(), 1_001).expect("purge"), 1);
}

#[test]
fn a_touched_file_is_the_same_media() {
    let dir = scratch_dir();
    let mut database = Database::in_memory();
    let path = dir.join("a.mkv");
    write(&path, b"unchanged");
    let id = id_of(&mut database, &path);
    let later = SystemTime::now() + Duration::from_secs(3_600);
    File::options().write(true).open(&path).and_then(|f| f.set_modified(later)).expect("touch");
    assert_eq!(id_of(&mut database, &path), id);
}

#[test]
fn urls_and_missing_files() {
    let mut database = Database::in_memory();
    let url = Media::Url("https://radio.example/live".into());
    let id = resolve(database.connection(), &url, 0).expect("a URL resolves");
    assert_eq!(resolve(database.connection(), &url, 0).expect("again"), id);
    let missing = Media::File(scratch_dir().join("gone.mkv"));
    let result = resolve(database.connection(), &missing, 0);
    assert!(matches!(result, Err(ResolveError::Io(_))), "{result:?}");
}

/// Runs every job sent so far.
fn drain(worker: &mut Worker, jobs: &Receiver<Job>) {
    while let Ok(job) = jobs.try_recv() {
        worker.handle(job);
    }
}

fn reply<T>(mut receiver: oneshot::Receiver<T>) -> T {
    receiver.try_recv().expect("not cancelled").expect("replied")
}

#[test]
fn sessions_save_through_the_history_thread() {
    let dir = scratch_dir();
    let path = dir.join("film.mkv");
    write(&path, b"a film");
    let (sender, jobs) = mpsc::channel();
    let history = History::new(sender);
    let mut worker = Worker::new(Database::in_memory());

    let (mut session, saved) = history.begin(Media::File(path.clone()));
    drain(&mut worker, &jobs);
    assert_eq!(reply(saved), Some(Saved::default()));
    session.played();
    let hour = Some(Duration::from_secs(3_600));
    session.update(Progress {
        position: Some(Duration::from_secs(60)),
        duration: hour,
        watched: false,
    });
    session.save();
    // The last progress is saved when the session ends, even unsaved.
    session.update(Progress {
        position: Some(Duration::from_secs(754)),
        duration: hour,
        watched: false,
    });
    drop(session);

    let (session, saved) = history.begin(Media::File(path));
    drain(&mut worker, &jobs);
    let expected = Saved {
        duration: hour,
        position: Some(Duration::from_secs(754)),
        watched: false,
        play_count: 1,
    };
    assert_eq!(reply(saved), Some(expected));
    session.discard();
}

#[test]
fn marking_and_clearing() {
    let dir = scratch_dir();
    let path = dir.join("film.mkv");
    write(&path, b"a film");
    let (sender, jobs) = mpsc::channel();
    let history = History::new(sender);
    let mut worker = Worker::new(Database::in_memory());
    let film = Media::File(path);

    history.set_watched(film.clone(), true);
    let (session, saved) = history.begin(film.clone());
    drain(&mut worker, &jobs);
    assert!(reply(saved).is_some_and(|saved| saved.watched));
    session.discard();

    history.clear();
    let (session, saved) = history.begin(film);
    drain(&mut worker, &jobs);
    assert_eq!(reply(saved), Some(Saved::default()));
    session.discard();
}

#[test]
fn media_that_cant_be_identified_has_no_history() {
    let (sender, jobs) = mpsc::channel();
    let history = History::new(sender);
    let mut worker = Worker::new(Database::in_memory());
    let (mut session, saved) = history.begin(Media::File(scratch_dir().join("gone.mkv")));
    session.update(Progress { watched: true, ..Progress::default() });
    session.save();
    drain(&mut worker, &jobs);
    assert_eq!(reply(saved), None);
}

#[test]
fn a_renamed_file_keeps_its_identity_without_a_row_being_written() {
    let dir = scratch_dir();
    let mut database = Database::in_memory();
    let (before, after) = (dir.join("a.mkv"), dir.join("renamed.mkv"));
    write(&before, b"bookmarked");
    let identity = |database: &mut Database, path: &Path| {
        let media = Media::File(path.to_path_buf());
        identity_of(database.connection(), &media).expect("the file is readable")
    };
    let first = identity(&mut database, &before);
    fs::rename(&before, &after).expect("rename");
    assert_eq!(identity(&mut database, &after), first);
    let located = media::find_at(database.connection(), &after.to_string_lossy());
    assert_eq!(located.expect("query"), None, "nothing was written");
    let id = id_of(&mut database, &after);
    assert_eq!(identity(&mut database, &after), first, "a row gives the same identity");
    assert!(id.0 > 0);
}
