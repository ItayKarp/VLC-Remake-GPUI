//! The library's tables (S4.1): roots and their shared folders, files and the media they are,
//! metadata, and plays, visits and favorites.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::Connection;

use crate::db::activity::{self, Favorite, Place};
use crate::db::files::{self, Scanned};
use crate::db::folders::{self, FolderId};
use crate::db::media::{self, Entry, FileStamp, MediaId, Progress};
use crate::db::metadata::{self, MediaKind, Metadata, Resolution};
use crate::db::played;
use crate::db::playlists::{self, Stored};
use crate::db::roots::{self, RootKind, RootStatus};
use crate::db::{Database, now_ms};
use crate::library::path_key::Case;

const STAMP: FileStamp = FileStamp { size: 10, modified_ms: Some(1) };

fn media_row(connection: &Connection, identity: &str) -> MediaId {
    let entry =
        Entry { identity: identity.into(), location: format!("/m/{identity}"), stamp: None };
    media::insert(connection, &entry).expect("insert media")
}

fn put(connection: &Connection, folder: FolderId, name: &str, media: MediaId, case: Case) {
    let file = Scanned { name: OsStr::new(name), stamp: STAMP, media: Some(media), cloud: false };
    files::put(connection, folder, &file, case).expect("put file");
}

fn paths(found: Vec<files::File>) -> Vec<PathBuf> {
    found.into_iter().map(|file| file.path).collect()
}

#[test]
fn nested_roots_share_their_folders() {
    let mut database = Database::in_memory();
    let c = database.connection();
    let outer = roots::add(c, Path::new("/v"), RootKind::Video, Case::Sensitive, 1).expect("add");
    let outer_folder = folders::find(c, Path::new("/v"), Case::Sensitive).expect("find");
    let inner =
        roots::add(c, Path::new("/v/tv"), RootKind::Mixed, Case::Sensitive, 2).expect("add");
    let tv = folders::find(c, Path::new("/v/tv"), Case::Sensitive).expect("find").expect("added");
    let children = folders::children(c, outer_folder.expect("added")).expect("children");
    assert_eq!(children.iter().map(|f| f.id).collect::<Vec<_>>(), [tv]);
    assert_eq!(roots::add(c, Path::new("/v/tv"), RootKind::Video, Case::Sensitive, 3), Ok(inner));

    let id = media_row(c, "a");
    put(c, tv, "show.mkv", id, Case::Sensitive);
    // The outer root goes; the inner one keeps its folder and files, and the media its row.
    roots::remove(c, outer).expect("remove");
    let left = roots::list(c).expect("list");
    assert_eq!(
        left.iter().map(|root| (root.id, root.kind)).collect::<Vec<_>>(),
        [(inner, RootKind::Mixed)]
    );
    assert_eq!(paths(files::in_folder(c, tv).expect("files")), [PathBuf::from("/v/tv/show.mkv")]);
    roots::remove(c, inner).expect("remove");
    assert_eq!(files::of_media(c, id).expect("files"), []);
    assert!(media::find_identity(c, "a").expect("query").is_some());
}

#[test]
fn the_same_media_shows_in_both_trees() {
    let mut database = Database::in_memory();
    let c = database.connection();
    for root in ["/movies", "/backup"] {
        roots::add(c, Path::new(root), RootKind::Video, Case::Sensitive, 1).expect("add");
    }
    let id = media_row(c, "film");
    for root in ["/movies", "/backup"] {
        let folder =
            folders::find(c, Path::new(root), Case::Sensitive).expect("find").expect("root");
        put(c, folder, "film.mkv", id, Case::Sensitive);
    }
    let found = paths(files::of_media(c, id).expect("files"));
    assert_eq!(found, [PathBuf::from("/backup/film.mkv"), PathBuf::from("/movies/film.mkv")]);
}

#[test]
fn names_compare_as_the_root_s_file_system_does() {
    let mut database = Database::in_memory();
    let c = database.connection();
    let (sensitive, insensitive) = (Case::Sensitive, Case::Insensitive);
    roots::add(c, Path::new("/Linux"), RootKind::Video, sensitive, 1).expect("add");
    roots::add(c, Path::new("/Mac"), RootKind::Video, insensitive, 1).expect("add");
    assert!(folders::find(c, Path::new("/linux"), sensitive).expect("find").is_none());
    let mac = folders::find(c, Path::new("/MAC"), insensitive).expect("find").expect("same folder");
    let linux = folders::find(c, Path::new("/Linux"), sensitive).expect("find").expect("added");

    let id = media_row(c, "a");
    for name in ["Film.mkv", "film.mkv"] {
        put(c, linux, name, id, sensitive);
        put(c, mac, name, id, insensitive);
    }
    assert_eq!(files::in_folder(c, linux).expect("files").len(), 2);
    // One file, named as last scanned; the original spelling is what opens it.
    assert_eq!(paths(files::in_folder(c, mac).expect("files")), [PathBuf::from("/Mac/film.mkv")]);
}

#[test]
fn unicode_forms_meet_where_the_file_system_folds_them() {
    let mut database = Database::in_memory();
    let c = database.connection();
    let decomposed = Path::new("/Cafe\u{301}");
    roots::add(c, decomposed, RootKind::Music, Case::Insensitive, 1).expect("add");
    let found = folders::find(c, Path::new("/Caf\u{e9}"), Case::Insensitive).expect("find");
    assert_eq!(found.is_some(), cfg!(target_os = "macos"));
    let root = &roots::list(c).expect("list")[0];
    assert_eq!(root.path, decomposed, "the original bytes are kept for opening");
}

#[test]
fn roots_remember_their_status() {
    let mut database = Database::in_memory();
    let c = database.connection();
    let id = roots::add(c, Path::new("/usb"), RootKind::Music, Case::Insensitive, 1).expect("add");
    assert_eq!(roots::list(c).expect("list")[0].status, RootStatus::Ok);
    roots::set_status(c, id, RootStatus::Missing).expect("status");
    roots::set_scanned(c, id, 5).expect("scanned");
    let root = &roots::list(c).expect("list")[0];
    assert_eq!((root.status, root.case), (RootStatus::Missing, Case::Insensitive));
}

#[test]
fn metadata_round_trips_and_unknown_kinds_are_refused() {
    let mut database = Database::in_memory();
    let c = database.connection();
    let id = media_row(c, "a");
    assert_eq!(metadata::get(c, id).expect("get"), Metadata::default());
    let tags = Metadata {
        kind: MediaKind::Audio,
        title: Some("Song".into()),
        album_artist: Some("Band".into()),
        track_number: Some(3),
        disc_number: Some(1),
        year: Some(1999),
        resolution: Some(Resolution { width: 1920, height: 1080 }),
        ..Metadata::default()
    };
    metadata::set(c, id, &tags).expect("set");
    assert_eq!(metadata::get(c, id).expect("get"), tags);
    let refused = c.execute("UPDATE media SET kind = 'film' WHERE id = ?1", [id.0]);
    assert!(refused.is_err(), "the schema's CHECK keeps kinds closed");
}

#[test]
fn plays_and_visits_feed_home() {
    let mut database = Database::in_memory();
    let c = database.connection();
    let (song, other, video) = (media_row(c, "song"), media_row(c, "other"), media_row(c, "video"));
    let audio = |artist: &str| Metadata {
        kind: MediaKind::Audio,
        artist: Some(artist.into()),
        ..Metadata::default()
    };
    metadata::set(c, song, &audio("Band")).expect("set");
    metadata::set(c, other, &audio("Solo")).expect("set");
    for (id, at) in [(song, 1), (song, 2), (other, 3), (video, 4)] {
        media::record_play(c, id, at).expect("play");
    }
    // Files have a stamp; streams don't, and aren't listed.
    c.execute("UPDATE media SET size = 1", []).expect("stamp");
    let recent = played::recent(c, MediaKind::Audio, 10).expect("query");
    assert_eq!(recent.iter().map(|played| played.media).collect::<Vec<_>>(), [other, song]);
    assert_eq!(activity::frequent_artists(c, 4, 10).expect("query"), ["Band", "Solo"]);

    roots::add(c, Path::new("/v"), RootKind::Video, Case::Sensitive, 1).expect("add");
    let folder = folders::find(c, Path::new("/v"), Case::Sensitive).expect("find").expect("root");
    let playlist = playlists::insert(c, None, &Stored::default(), 1).expect("playlist");
    for place in [Place::Folder(folder), Place::Playlist(playlist), Place::Folder(folder)] {
        activity::record_visit(c, place, now_ms()).expect("visit");
    }
    assert_eq!(activity::frequent_folders(c, now_ms(), 5).expect("query"), [folder]);
    assert_eq!(activity::frequent_playlists(c, now_ms(), 5).expect("query"), [playlist]);
}

#[test]
fn a_habit_from_long_ago_ranks_below_a_recent_one() {
    const DAY_MS: i64 = 24 * 60 * 60 * 1000;
    let mut database = Database::in_memory();
    let c = database.connection();
    let (old, new) = (
        playlists::insert(c, None, &Stored::default(), 1).expect("playlist"),
        playlists::insert(c, None, &Stored::default(), 1).expect("playlist"),
    );
    for _ in 0..20 {
        activity::record_visit(c, Place::Playlist(old), 0).expect("visit");
    }
    for day in 360..365 {
        activity::record_visit(c, Place::Playlist(new), day * DAY_MS).expect("visit");
    }
    assert_eq!(activity::frequent_playlists(c, 365 * DAY_MS, 5).expect("query"), [new, old]);
    assert_eq!(activity::frequent_playlists(c, 0, 5).expect("query")[0], old);
}

#[test]
fn continue_watching_lists_partway_media_until_removed() {
    let mut database = Database::in_memory();
    let c = database.connection();
    let file = |identity: &str| {
        let entry = Entry {
            identity: identity.into(),
            location: format!("/v/{identity}.mkv"),
            stamp: Some(STAMP),
        };
        media::insert(c, &entry).expect("insert media")
    };
    let (partway, done, fresh) = (file("partway"), file("done"), file("fresh"));
    let progress = |seconds| Progress {
        position: Some(Duration::from_secs(seconds)),
        duration: Some(Duration::from_secs(100)),
        watched: false,
    };
    media::save_progress(c, partway, progress(30), 10).expect("save");
    media::save_progress(c, done, progress(40), 20).expect("save");
    media::set_watched(c, done, true).expect("watched");
    media::insert(c, &Entry { identity: "url:x".into(), location: "x".into(), stamp: None })
        .expect("stream");
    let listed = |c: &Connection| -> Vec<MediaId> {
        played::in_progress(c, 10).expect("query").iter().map(|played| played.media).collect()
    };
    assert_eq!(listed(c), [partway]);
    assert_eq!(played::in_progress(c, 10).expect("query")[0].path, Path::new("/v/partway.mkv"));
    assert_eq!(played::one(c, fresh).expect("query").map(|played| played.media), Some(fresh));

    played::hide_in_progress(c, partway, 15).expect("hide");
    assert_eq!(listed(c), []);
    assert_eq!(media::saved(c, partway).expect("kept").position, Some(Duration::from_secs(30)));
    media::save_progress(c, partway, progress(35), 16).expect("played again");
    assert_eq!(listed(c), [partway]);
}

#[test]
fn favorites_toggle_and_go_with_their_playlist() {
    let mut database = Database::in_memory();
    let c = database.connection();
    let playlist = playlists::insert(c, None, &Stored::default(), 1).expect("playlist");
    let favorite = Favorite::Playlist(playlist);
    activity::add_favorite(c, favorite, 1).expect("add");
    activity::add_favorite(c, favorite, 2).expect("again");
    assert_eq!(activity::is_favorite(c, favorite), Ok(true));
    activity::remove_favorite(c, favorite).expect("remove");
    assert_eq!(activity::is_favorite(c, favorite), Ok(false));
    activity::add_favorite(c, favorite, 3).expect("add");
    playlists::delete(c, playlist).expect("delete");
    assert_eq!(activity::is_favorite(c, favorite), Ok(false));
}

#[test]
fn clearing_history_keeps_the_library() {
    let mut database = Database::in_memory();
    let c = database.connection();
    roots::add(c, Path::new("/v"), RootKind::Video, Case::Sensitive, 1).expect("add");
    let folder = folders::find(c, Path::new("/v"), Case::Sensitive).expect("find").expect("root");
    let (indexed, loved, opened) =
        (media_row(c, "indexed"), media_row(c, "loved"), media_row(c, "opened"));
    put(c, folder, "a.mkv", indexed, Case::Sensitive);
    activity::add_favorite(c, Favorite::Media(loved), 1).expect("favorite");
    metadata::set(c, indexed, &Metadata { title: Some("A".into()), ..Metadata::default() })
        .expect("set");
    for id in [indexed, loved, opened] {
        media::record_play(c, id, 5).expect("play");
        media::set_watched(c, id, true).expect("watched");
    }
    activity::record_visit(c, Place::Folder(folder), 5).expect("visit");

    media::clear(c).expect("clear");
    assert_eq!(media::find_identity(c, "opened").expect("query"), None);
    let saved = media::saved(c, indexed).expect("kept");
    assert_eq!((saved.watched, saved.play_count), (false, 0));
    assert_eq!(metadata::get(c, indexed).expect("tags").title.as_deref(), Some("A"));
    assert_eq!(activity::is_favorite(c, Favorite::Media(loved)), Ok(true));
    assert_eq!(played::recent(c, MediaKind::Unknown, 5).expect("query"), []);
    assert_eq!(activity::frequent_folders(c, now_ms(), 5).expect("query"), []);
}
