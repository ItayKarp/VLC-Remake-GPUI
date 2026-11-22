//! Opening, recovering and migrating the database, and the `media` table. The library's tables
//! are in [`library`].

mod library;

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use super::media::{self, Entry, FileStamp, Progress, Saved};
use super::migrations::{self, CURRENT_VERSION, MIGRATIONS};
use super::playlists::{self, Stored};
use super::{CORRUPT_SUFFIX, Database};
use crate::opening::scratch::scratch_dir;

fn db_path() -> PathBuf {
    scratch_dir().join("library.db")
}

fn entry(identity: &str) -> Entry {
    let stamp = FileStamp { size: 10, modified_ms: Some(1) };
    Entry { identity: identity.into(), location: format!("/m/{identity}"), stamp: Some(stamp) }
}

fn secs(secs: u64) -> Duration {
    Duration::from_secs(secs)
}

#[test]
fn a_new_database_is_created_and_migrated() {
    let path = db_path();
    let mut database = Database::open(&path);
    assert!(path.exists());
    let version = migrations::version(database.connection()).map_err(|e| e.to_string());
    assert_eq!(version, Ok(CURRENT_VERSION));
    drop(database);
    // Reopening runs nothing again, and keeps what was written.
    let mut database = Database::open(&path);
    media::insert(database.connection(), &entry("a")).expect("insert");
    drop(database);
    let mut database = Database::open(&path);
    assert!(media::find_identity(database.connection(), "a").expect("query").is_some());
}

#[test]
fn a_damaged_database_is_moved_aside_and_started_over() {
    let path = db_path();
    fs::write(&path, vec![0xAB; 8192]).expect("write garbage");
    let mut database = Database::open(&path);
    media::insert(database.connection(), &entry("a")).expect("the new database works");
    let backup = super::with_suffix(&path, CORRUPT_SUFFIX);
    assert_eq!(fs::read(backup).expect("backup kept"), vec![0xAB; 8192]);
}

#[test]
fn a_database_from_a_newer_build_is_left_alone() {
    let path = db_path();
    let mut database = Database::open(&path);
    database.connection().pragma_update(None, "user_version", CURRENT_VERSION + 1).expect("bump");
    drop(database);
    let before = fs::read(&path).expect("read");
    let mut database = Database::open(&path);
    // History works, in memory, and the file is untouched.
    media::insert(database.connection(), &entry("a")).expect("in memory");
    assert_eq!(fs::read(&path).expect("read"), before);
    assert!(!super::with_suffix(&path, CORRUPT_SUFFIX).exists());
}

#[test]
fn progress_plays_and_watched_are_saved() {
    let mut database = Database::in_memory();
    let connection = database.connection();
    let id = media::insert(connection, &entry("a")).expect("insert");
    assert_eq!(media::saved(connection, id).expect("saved"), Saved::default());

    media::record_play(connection, id, 100).expect("play");
    let progress =
        Progress { position: Some(secs(754)), duration: Some(secs(3_600)), watched: false };
    media::save_progress(connection, id, progress, 200).expect("save");
    let saved = media::saved(connection, id).expect("saved");
    let expected = Saved {
        duration: Some(secs(3_600)),
        position: Some(secs(754)),
        watched: false,
        play_count: 1,
    };
    assert_eq!(saved, expected);

    // A save without a duration keeps the known one; watched is never unmarked by a save.
    let watched = Progress { position: None, duration: None, watched: true };
    media::save_progress(connection, id, watched, 300).expect("save");
    media::save_progress(connection, id, Progress::default(), 400).expect("save");
    let saved = media::saved(connection, id).expect("saved");
    assert_eq!((saved.duration, saved.position, saved.watched), (Some(secs(3_600)), None, true));

    media::set_watched(connection, id, false).expect("unmark");
    assert!(!media::saved(connection, id).expect("saved").watched);
}

#[test]
fn orphans_are_found_last_and_purged_when_old() {
    let mut database = Database::in_memory();
    let connection = database.connection();
    let old = media::insert(connection, &entry("a")).expect("insert");
    media::orphan(connection, old, 1_000).expect("orphan");
    assert_eq!(media::find_at(connection, "/m/a").expect("query"), None);
    let live = media::insert(connection, &entry("a")).expect("insert");
    assert_eq!(media::find_identity(connection, "a").expect("query"), Some(live));

    assert_eq!(media::purge_orphans(connection, 1_000).expect("purge"), 0);
    assert_eq!(media::purge_orphans(connection, 1_001).expect("purge"), 1);
    media::clear(connection).expect("clear");
    assert_eq!(media::find_identity(connection, "a").expect("query"), None);
}

#[test]
fn a_database_from_every_older_version_is_brought_up_to_date() {
    for version in 1..CURRENT_VERSION {
        let path = db_path();
        let connection = rusqlite::Connection::open(&path).expect("open");
        for sql in &MIGRATIONS[..version as usize] {
            connection.execute_batch(sql).expect("an older schema");
        }
        connection.pragma_update(None, "user_version", version).expect("set version");
        connection
            .execute(
                "INSERT INTO media (identity, location, play_count, last_played_ms)
                 VALUES ('a', '/m/a', 2, 500)",
                [],
            )
            .expect("an older row");
        drop(connection);

        let mut database = Database::open(&path);
        let migrated = migrations::version(database.connection()).map_err(|e| e.to_string());
        assert_eq!(migrated, Ok(CURRENT_VERSION), "from version {version}");
        let id = media::find_identity(database.connection(), "a").expect("query").expect("kept");
        assert_eq!(media::saved(database.connection(), id).expect("saved").play_count, 2);
        if version < 3 {
            assert_migrated_to_the_library(database.connection(), id);
        }
        let playlist = Stored { title: "Mix".into(), ..Stored::default() };
        playlists::insert(database.connection(), None, &playlist, 1).expect("the new tables");
        let pending = super::files::unidentified(database.connection(), super::files::FileId(0), 1);
        assert_eq!(pending.expect("the indexing columns"), []);
    }
}

/// What migration 3 makes of a row from before the library: added when last played, its plays
/// kept for Listen Again, not probed yet.
fn assert_migrated_to_the_library(connection: &rusqlite::Connection, id: media::MediaId) {
    let (added, kind, plays): (i64, String, i64) = connection
        .query_row(
            "SELECT added_ms, kind, (SELECT COUNT(*) FROM plays WHERE media_id = id)
             FROM media WHERE id = ?1",
            [id.0],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("library columns");
    assert_eq!((added, kind.as_str(), plays), (500, "unknown", 1));
}

#[test]
fn playlists_of_links_only_are_streams() {
    let mut database = Database::in_memory();
    let connection = database.connection();
    let stored = |title: &str, locations: &[&str]| Stored {
        title: title.into(),
        thumbnail: None,
        locations: locations.iter().map(|location| (*location).to_owned()).collect(),
    };
    let lists = [
        stored("a", &["https://x/live", "rtsp://cam/1"]),
        stored("b", &["https://x/live", "/m/a.mkv"]),
        stored("c", &[]),
    ];
    for list in &lists {
        playlists::insert(connection, None, list, 1).expect("insert");
    }
    let streams: Vec<bool> =
        playlists::list(connection).expect("list").iter().map(|s| s.streams_only).collect();
    assert_eq!(streams, [true, false, false]);
}
