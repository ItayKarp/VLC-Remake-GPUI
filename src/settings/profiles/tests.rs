use super::book::{self, DESIGNED};
use crate::settings::presets::Layer;
use crate::settings::{Automatic, Preferences, Preset, StoredLayout};

/// A layout that isn't the designed one.
fn custom_layout() -> StoredLayout {
    StoredLayout { controls: vec![vec!["play-pause".into()]], ..StoredLayout::default() }
}

/// Preferences with preset "Anime" (bold subtitles).
fn with_preset() -> Preferences {
    let mut prefs = Preferences::default();
    let anime = toml::toml! { [subtitles] bold = true };
    prefs.presets.list = vec![Preset { name: "Anime".into(), values: anime }];
    prefs
}

#[test]
fn a_new_user_has_the_designed_four_with_the_first_in_use() {
    let prefs = Preferences::default();
    let names: Vec<String> = book::list(&prefs).iter().map(|p| p.name.clone()).collect();
    assert_eq!(names, DESIGNED);
    assert_eq!(book::active(&prefs), "Movies");
}

#[test]
fn switching_keeps_each_profiles_layout_and_preset() {
    let mut prefs = with_preset();
    let mut layer = Layer::default();
    prefs.player_layout = custom_layout();
    layer.select(&mut prefs, Some("Anime"));

    assert!(book::select(&mut layer, &mut prefs, "Shows"));
    assert_eq!(prefs.player_layout, StoredLayout::default(), "Shows has the designed layout");
    assert_eq!(prefs.presets.active, "");
    assert!(!prefs.subtitles.bold, "Default Preset is back");

    assert!(book::select(&mut layer, &mut prefs, "Movies"));
    assert_eq!(prefs.player_layout, custom_layout());
    assert_eq!(prefs.presets.active, "Anime");
    assert!(prefs.subtitles.bold);
    assert!(!book::select(&mut layer, &mut prefs, "Nothing"));
}

#[test]
fn a_new_profile_copies_the_one_in_use_and_takes_a_free_name() {
    let mut prefs = with_preset();
    let mut layer = Layer::default();
    prefs.player_layout = custom_layout();
    layer.select(&mut prefs, Some("Anime"));
    book::set_automatic(&mut prefs, "Movies", Automatic::Video);

    assert_eq!(book::create(&mut layer, &mut prefs, " movies "), "movies (2)");
    assert_eq!(book::active(&prefs), "movies (2)");
    assert_eq!(prefs.player_layout, custom_layout());
    assert_eq!(prefs.presets.active, "Anime");
    assert_eq!(book::for_media(&prefs, Automatic::Video).as_deref(), Some("Movies"));
    assert_eq!(book::create(&mut layer, &mut prefs, ""), book::NEW);
}

#[test]
fn renaming_follows_the_profile_in_use_and_refuses_empty_names() {
    let mut prefs = Preferences::default();
    assert_eq!(book::rename(&mut prefs, "Movies", "Films").as_deref(), Some("Films"));
    assert_eq!(book::active(&prefs), "Films");
    assert_eq!(book::rename(&mut prefs, "Films", "  "), None);
    assert_eq!(book::rename(&mut prefs, "Shows", "films").as_deref(), Some("films (2)"));
}

#[test]
fn deleting_the_profile_in_use_switches_to_a_neighbour_and_the_last_stays() {
    let mut prefs = Preferences::default();
    let mut layer = Layer::default();
    prefs.player_layout = custom_layout();

    assert!(book::delete(&mut layer, &mut prefs, "Movies"));
    assert_eq!(book::active(&prefs), "Shows");
    assert_eq!(prefs.player_layout, StoredLayout::default(), "Shows' layout shows");
    for name in ["Shows", "Music"] {
        assert!(book::delete(&mut layer, &mut prefs, name));
    }
    assert!(!book::delete(&mut layer, &mut prefs, "Work"), "the last profile stays");
    assert_eq!(book::active(&prefs), "Work");
}

#[test]
fn each_kind_of_media_switches_to_one_profile_at_most() {
    let mut prefs = Preferences::default();
    book::set_automatic(&mut prefs, "Music", Automatic::Music);
    book::set_automatic(&mut prefs, "Work", Automatic::Music);
    assert_eq!(book::for_media(&prefs, Automatic::Music).as_deref(), Some("Work"));
    book::set_automatic(&mut prefs, "Work", Automatic::Never);
    assert_eq!(book::for_media(&prefs, Automatic::Music), None);
    assert_eq!(book::for_media(&prefs, Automatic::Never), None);
}

#[test]
fn a_renamed_or_deleted_preset_follows_into_the_profiles() {
    let mut prefs = with_preset();
    let mut layer = Layer::default();
    layer.select(&mut prefs, Some("Anime"));
    book::select(&mut layer, &mut prefs, "Shows");

    book::preset_renamed(&mut prefs, "Anime", "Cartoons");
    let movies = |prefs: &Preferences| book::list(prefs)[0].preset.clone();
    assert_eq!(movies(&prefs), "Cartoons");
    book::preset_deleted(&mut prefs, "Cartoons");
    assert_eq!(movies(&prefs), "");
}

#[test]
fn profiles_survive_the_settings_file() {
    let mut prefs = Preferences::default();
    let mut layer = Layer::default();
    prefs.player_layout = custom_layout();
    book::create(&mut layer, &mut prefs, "Late Night");
    book::set_automatic(&mut prefs, "Late Night", Automatic::Video);

    let text = toml::to_string(&prefs.profiles).expect("serializes");
    let read: crate::settings::ProfileSettings = toml::from_str(&text).expect("parses");
    assert_eq!(read, prefs.profiles);
}
