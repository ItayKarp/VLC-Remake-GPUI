use std::time::Duration;

use super::*;
use crate::db::Database;
use crate::db::activity::Place;
use crate::db::media::{self, Entry, FileStamp, MediaId, Progress};
use crate::db::metadata::{self, Metadata};
use crate::db::playlists::{self, Stored};
use crate::settings::Settings;

fn wanted(history: bool) -> Wanted {
    let rule = WatchedRule::from_settings(&Settings::default().prefs.player);
    Wanted { rule, history, now_ms: 1_000 }
}

fn file(connection: &Connection, path: &str) -> MediaId {
    let stamp = Some(FileStamp { size: 1, modified_ms: None });
    let entry = Entry { identity: path.into(), location: path.into(), stamp };
    media::insert(connection, &entry).expect("insert media")
}

fn left_at(connection: &Connection, id: MediaId, seconds: u64, at: i64) {
    let progress = Progress {
        position: Some(Duration::from_secs(seconds)),
        duration: Some(Duration::from_secs(3_600)),
        watched: false,
    };
    media::save_progress(connection, id, progress, at).expect("save");
}

#[test]
fn partway_videos_continue_by_file_name_the_latest_first() {
    let mut database = Database::in_memory();
    let c = database.connection();
    let (first, second) = (file(c, "/v/The_Wand_S01E05.mp4"), file(c, "/v/The_Wand_S01E06.mp4"));
    let nearly_done = file(c, "/v/done.mp4");
    left_at(c, first, 600, 10);
    left_at(c, second, 900, 20);
    left_at(c, nearly_done, 3_590, 30);
    let home = read(c, wanted(true)).expect("read");
    let labels: Vec<&str> = home.continue_watching.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, ["The_Wand_S01E06.mp4", "The_Wand_S01E05.mp4"]);
}

#[test]
fn songs_say_their_artist() {
    let mut database = Database::in_memory();
    let c = database.connection();
    let song = file(c, "/m/untangle.mp3");
    let tags = Metadata {
        kind: MediaKind::Audio,
        title: Some("untangle".into()),
        artist: Some("juxtopposed".into()),
        ..Metadata::default()
    };
    metadata::set(c, song, &tags).expect("tags");
    media::record_play(c, song, 5).expect("play");
    let home = read(c, wanted(true)).expect("read");
    assert_eq!(home.listen_again[0].label, "untangle - juxtopposed");
    assert_eq!(home.artists, ["juxtopposed"]);
}

#[test]
fn without_history_only_favorites_are_read() {
    let mut database = Database::in_memory();
    let c = database.connection();
    let video = file(c, "/v/a.mp4");
    left_at(c, video, 600, 10);
    let (older, newer) = (
        playlists::insert(c, None, &Stored::default(), 1).expect("playlist"),
        playlists::insert(c, None, &Stored::default(), 1).expect("playlist"),
    );
    activity::add_favorite(c, Favorite::Playlist(older), 1).expect("favorite");
    activity::add_favorite(c, Favorite::Playlist(newer), 2).expect("favorite");
    activity::add_favorite(c, Favorite::Media(video), 3).expect("favorite");
    activity::record_visit(c, Place::Playlist(older), 4).expect("visit");

    let home = read(c, wanted(false)).expect("read");
    assert_eq!(home.favorite_playlists, [newer, older]);
    assert_eq!(home.favorite_media.len(), 1);
    assert!(home.continue_watching.is_empty() && home.playlists.is_empty());
    assert_eq!(read(c, wanted(true)).expect("read").playlists, [older]);
}

#[test]
fn missing_files_are_counted_not_listed() {
    let here = std::env::temp_dir();
    let card = |path: PathBuf| Card {
        label: String::new(),
        path,
        media: None,
        kind: MediaKind::Video,
        watch: WatchState::InProgress(0.5),
        tags: Metadata::default(),
        added_ms: None,
    };
    let mut home = Home {
        continue_watching: vec![card(here.clone()), card(here.join("no such file.mkv"))],
        ..Home::default()
    };
    home.drop_unavailable();
    assert_eq!((home.continue_watching.len(), home.unavailable), (1, 1));
}
