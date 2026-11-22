use std::path::PathBuf;

use super::*;
use crate::db::playlists::PlaylistId;
use crate::navigation::route::Search;

fn labels(trail: &Trail) -> Vec<&str> {
    trail.crumbs.iter().map(|crumb| crumb.label.as_str()).collect()
}

fn with<R>(f: impl FnOnce(Places) -> R) -> R {
    let videos = [PathBuf::from("/media/Shows"), PathBuf::from("/media/Shows/Anime")];
    let music = [PathBuf::from("/music/My Songs")];
    let playlists = [Summary { id: PlaylistId(4), title: "Cool Songs".into(), len: 2 }];
    f(Places { video_roots: &videos, music_roots: &music, playlists: &playlists })
}

#[test]
fn a_section_root_is_one_segment() {
    let trail = with(|places| Trail::of(&Route::Videos(None), places));
    assert_eq!(labels(&trail), ["Videos"]);
    assert_eq!(trail.crumbs[0].route, Route::Videos(None));
}

#[test]
fn a_video_folder_goes_through_its_media_folder() {
    let folder = PathBuf::from("/media/Shows/The Wand/S01");
    let trail = with(|places| Trail::of(&Route::Videos(Some(folder.clone())), places));
    assert_eq!(labels(&trail), ["Videos", "Shows", "The Wand", "S01"]);
    assert_eq!(trail.crumbs[2].route, Route::Videos(Some("/media/Shows/The Wand".into())));
    assert_eq!(trail.crumbs[3].route, Route::Videos(Some(folder)));
    assert!(!trail.outside_library);
}

#[test]
fn the_innermost_media_folder_names_the_way() {
    let folder = PathBuf::from("/media/Shows/Anime/Mob");
    let trail = with(|places| Trail::of(&Route::Videos(Some(folder)), places));
    assert_eq!(labels(&trail), ["Videos", "Anime", "Mob"]);
}

#[test]
fn a_songs_folder_goes_through_music_and_songs() {
    let route = Route::Music(MusicRoute::Songs(Some("/music/My Songs".into())));
    let trail = with(|places| Trail::of(&route, places));
    assert_eq!(labels(&trail), ["Music", "Songs", "My Songs"]);
}

#[cfg(unix)]
#[test]
fn a_folder_outside_the_library_shows_its_real_path() {
    let folder = PathBuf::from("/Users/me/Downloads");
    let trail = with(|places| Trail::of(&Route::Videos(Some(folder)), places));
    assert!(trail.outside_library);
    assert_eq!(labels(&trail), ["/", "Users", "me", "Downloads"]);
    assert_eq!(trail.crumbs[0].route, Route::Videos(Some("/".into())));
}

#[test]
fn named_places_end_with_their_name() {
    let album = Route::Music(MusicRoute::Albums(Some("Juxt".into())));
    assert_eq!(labels(&with(|places| Trail::of(&album, places))), ["Music", "Albums", "Juxt"]);
    let playlist = Route::Playlists(Some(PlaylistId(4)));
    assert_eq!(labels(&with(|places| Trail::of(&playlist, places))), ["Playlists", "Cool Songs"]);
}

#[test]
fn a_search_names_its_scope_and_query() {
    let search = |scope| Route::Search(Search { query: "wand".into(), scope });
    let everywhere = with(|places| Trail::of(&search(Section::Home), places));
    assert_eq!(labels(&everywhere), ["Results for “wand”"]);
    let videos = with(|places| Trail::of(&search(Section::Videos), places));
    assert_eq!(labels(&videos), ["Videos", "Results for “wand”"]);
}

#[test]
fn three_levels_deep_collapses_to_parent_and_current() {
    let deep = Route::Videos(Some("/media/Shows/The Wand/S01".into()));
    let trail = with(|places| Trail::of(&deep, places));
    let (hidden, shown) = trail.collapsed();
    let names = |crumbs: &[Crumb]| crumbs.iter().map(|c| c.label.clone()).collect::<Vec<_>>();
    assert_eq!(names(hidden), ["Videos", "Shows"]);
    assert_eq!(names(shown), ["The Wand", "S01"]);
}

#[test]
fn two_levels_hide_nothing() {
    let trail = with(|places| Trail::of(&MusicCategory::Songs.route(), places));
    let (hidden, shown) = trail.collapsed();
    assert_eq!(hidden, &[] as &[Crumb]);
    assert_eq!(shown.len(), 2);
}
