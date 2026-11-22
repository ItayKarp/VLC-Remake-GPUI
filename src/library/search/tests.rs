use std::ffi::OsStr;
use std::path::Path;

use super::*;
use crate::db::Database;
use crate::db::files::{self, Scanned};
use crate::db::folders;
use crate::db::media::{self, Entry, FileStamp, MediaId};
use crate::db::metadata::{self, Metadata};
use crate::db::roots::{self, RootKind};
use crate::library::path_key::Case;
use crate::settings::Settings;

fn rule() -> WatchedRule {
    WatchedRule::from_settings(&Settings::default().prefs.player)
}

/// A library of `/m` holding `files`, each a media with `tags` (or none).
fn library(files: &[(&str, Option<Metadata>)]) -> Database {
    let mut database = Database::in_memory();
    let c = database.connection();
    roots::add(c, Path::new("/m"), RootKind::Video, Case::Sensitive, 1).expect("add");
    let folder = folders::find(c, Path::new("/m"), Case::Sensitive).expect("find").expect("root");
    for (name, tags) in files {
        let media = tags.as_ref().map(|tags| {
            let entry =
                Entry { identity: (*name).into(), location: format!("/m/{name}"), stamp: None };
            let id = media::insert(c, &entry).expect("insert");
            metadata::set(c, id, tags).expect("tags");
            id
        });
        let stamp = FileStamp { size: 1, modified_ms: None };
        let file = Scanned { name: OsStr::new(name), stamp, media, cloud: false };
        files::put(c, folder, &file, Case::Sensitive).expect("put");
    }
    database
}

fn song(title: &str, artist: &str) -> Metadata {
    Metadata {
        kind: MediaKind::Audio,
        title: Some(title.into()),
        artist: Some(artist.into()),
        ..Metadata::default()
    }
}

fn found(database: &mut Database, typed: &str, wanted: Option<MediaKind>) -> Vec<String> {
    let query = Query::parse(typed).expect("a query");
    let cards = find(database.connection(), &query, wanted, rule()).expect("search");
    cards.into_iter().map(|card| card.label).collect()
}

#[test]
fn queries_are_trimmed_capped_and_need_a_letter_or_digit() {
    assert_eq!(Query::parse("  "), None);
    assert_eq!(Query::parse(" -- !? "), None);
    assert_eq!(
        Query::parse(" Café  Noir ").map(|q| q.words),
        Some(vec!["cafe".into(), "noir".into()])
    );
    assert_eq!(clean(&"x".repeat(500)).len(), MAX_QUERY_CHARS);
    assert_eq!(clean("  wand "), "wand");
}

#[test]
fn searches_ignore_case_and_accents_in_names_and_tags() {
    let mut database =
        library(&[("Beyonce.mp3", Some(song("Halo", "Beyoncé"))), ("plain.mkv", None)]);
    assert_eq!(found(&mut database, "BEYONCE", None), ["Halo"]);
    assert_eq!(found(&mut database, "beyoncé halo", None), ["Halo"]);
    assert_eq!(found(&mut database, "PLAIN", None), ["plain.mkv"]);
    assert_eq!(found(&mut database, "nothing", None), Vec::<String>::new());
}

#[test]
fn short_words_and_text_without_spaces_match_inside_words() {
    let mut database =
        library(&[("千と千尋の神隠し.mkv", None), ("Up.mkv", None), ("Upgrade.mkv", None)]);
    assert_eq!(found(&mut database, "千尋", None), ["千と千尋の神隠し.mkv"]);
    assert_eq!(found(&mut database, "神隠し", None), ["千と千尋の神隠し.mkv"]);
    assert_eq!(found(&mut database, "up", None), ["Up.mkv", "Upgrade.mkv"]);
    assert_eq!(found(&mut database, "up grade", None), ["Upgrade.mkv"]);
}

#[test]
fn a_scope_keeps_its_own_kind() {
    let mut database = library(&[("song.mp3", Some(song("Song", "X"))), ("film.mkv", None)]);
    assert_eq!(found(&mut database, "s", None).len(), 1);
    assert_eq!(found(&mut database, "o", Some(MediaKind::Audio)), ["Song"]);
    assert_eq!(found(&mut database, "film", Some(MediaKind::Audio)), Vec::<String>::new());
}

#[test]
fn the_index_follows_renames_tag_changes_and_removals() {
    let mut database = library(&[("a.mp3", Some(song("Old Title", "X")))]);
    let media = MediaId(1);
    let renamed = Metadata { title: Some("New Title".into()), ..song("", "X") };
    metadata::set(database.connection(), media, &renamed).expect("retag");
    assert_eq!(found(&mut database, "new", None), ["New Title"]);
    assert_eq!(found(&mut database, "old", None), Vec::<String>::new());
    let file = files::of_media(database.connection(), media).expect("files").remove(0);
    files::remove(database.connection(), file.id).expect("remove");
    assert_eq!(found(&mut database, "new", None), Vec::<String>::new());
}

#[test]
fn a_media_in_two_places_is_found_once() {
    let mut database = library(&[("a.mp3", Some(song("Halo", "X")))]);
    let c = database.connection();
    let folder = folders::find(c, Path::new("/m"), Case::Sensitive).expect("find").expect("root");
    let stamp = FileStamp { size: 1, modified_ms: None };
    let copy =
        Scanned { name: OsStr::new("copy.mp3"), stamp, media: Some(MediaId(1)), cloud: false };
    files::put(c, folder, &copy, Case::Sensitive).expect("put");
    assert_eq!(found(&mut database, "halo", None), ["Halo"]);
}
