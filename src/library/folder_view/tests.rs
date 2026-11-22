use std::ffi::OsStr;
use std::time::Duration;

use super::*;
use crate::db::Database;
use crate::db::files::Scanned;
use crate::db::media::{self, Entry, FileStamp, Progress, Saved};
use crate::db::roots::{self, RootKind};
use crate::opening::scratch::scratch_dir;
use crate::settings::Settings;

fn rule() -> WatchedRule {
    WatchedRule::from_settings(&Settings::default().prefs.player)
}

fn listed(name: &str, kind: MediaKind, saved: Saved) -> Listed {
    let metadata = Metadata { kind, ..Metadata::default() };
    Listed { name: name.into(), media: Some(MediaId(1)), saved, metadata, added_ms: None }
}

fn names(cards: &[Card]) -> Vec<String> {
    cards.iter().map(|card| card.label.clone()).collect()
}

#[test]
fn cards_are_in_natural_order() {
    let files = ["/tv/E10.mkv", "/tv/e2.mkv", "/tv/E1.mkv"].map(PathBuf::from).to_vec();
    let cards = cards(files, Vec::new(), MediaKind::Video, rule());
    assert_eq!(names(&cards), ["E1.mkv", "e2.mkv", "E10.mkv"]);
    let mut folders = ["/tv/S10", "/tv/S2"].map(PathBuf::from).to_vec();
    sort_folders(&mut folders);
    assert_eq!(folders, ["/tv/S2", "/tv/S10"].map(PathBuf::from));
}

#[test]
fn each_section_lists_its_own_kind() {
    let files = ["/m/a.mkv", "/m/b.mp3", "/m/c.xyz", "/m/d.ogg"].map(PathBuf::from).to_vec();
    // The library probed `d.ogg` as a video, which wins over its extension.
    let known = vec![listed("d.ogg", MediaKind::Video, Saved::default())];
    let videos = cards(files.clone(), known.clone(), MediaKind::Video, rule());
    assert_eq!(names(&videos), ["a.mkv", "c.xyz", "d.ogg"]);
    let songs = cards(files, known, MediaKind::Audio, rule());
    assert_eq!(names(&songs), ["b.mp3", "c.xyz"]);
}

#[test]
fn cards_carry_the_library_s_media_and_watch_state() {
    let watched = Saved { watched: true, ..Saved::default() };
    let half = Saved {
        position: Some(Duration::from_mins(30)),
        duration: Some(Duration::from_hours(1)),
        ..Saved::default()
    };
    let known =
        vec![listed("a.mkv", MediaKind::Video, watched), listed("b.mkv", MediaKind::Video, half)];
    let files = ["/v/a.mkv", "/v/b.mkv", "/v/c.mkv"].map(PathBuf::from).to_vec();
    let cards = cards(files, known, MediaKind::Video, rule());
    let states: Vec<WatchState> = cards.iter().map(|card| card.watch).collect();
    assert_eq!(states, [WatchState::Watched, WatchState::InProgress(0.5), WatchState::Unwatched]);
    assert_eq!(cards[0].media, Some(MediaId(1)));
    assert_eq!(cards[2].media, None);
}

#[test]
fn the_disk_lists_folders_and_media_only() {
    let dir = scratch_dir();
    std::fs::create_dir(dir.join("S01")).expect("folder");
    std::fs::create_dir(dir.join(".hidden")).expect("folder");
    std::fs::write(dir.join("a.mkv"), b"x").expect("file");
    std::fs::write(dir.join("notes.txt"), b"x").expect("file");
    let on_disk = read_disk(&dir, false).expect("read");
    assert_eq!(on_disk.folders, [dir.join("S01")]);
    assert_eq!(on_disk.files, [dir.join("a.mkv")]);
    assert_eq!(read_disk(&dir, true).expect("read").folders.len(), 2);
    assert!(read_disk(&dir.join("gone"), false).is_err());
}

#[test]
fn the_library_s_files_are_read_with_their_history() {
    let mut database = Database::in_memory();
    let c = database.connection();
    roots::add(c, Path::new("/v"), RootKind::Video, Case::Insensitive, 1).expect("add");
    let folder = folders::find(c, Path::new("/V"), Case::Insensitive).expect("find").expect("root");
    let entry = Entry { identity: "a".into(), location: "/v/A.mkv".into(), stamp: None };
    let id = media::insert(c, &entry).expect("insert");
    let progress = Progress { watched: true, ..Progress::default() };
    media::save_progress(c, id, progress, 2).expect("save");
    let stamp = FileStamp { size: 1, modified_ms: None };
    for (name, media) in [("A.mkv", Some(id)), ("b.mkv", None)] {
        let file = Scanned { name: OsStr::new(name), stamp, media, cloud: false };
        files::put(c, folder, &file, Case::Insensitive).expect("put");
    }
    let mut known = read_library(c, Path::new("/V")).expect("read");
    known.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(known.len(), 2);
    assert_eq!((known[0].media, known[0].saved.watched), (Some(id), true));
    assert_eq!((known[1].media, known[1].metadata.kind), (None, MediaKind::Unknown));
    assert_eq!(read_library(c, Path::new("/elsewhere")).expect("read"), []);
}
