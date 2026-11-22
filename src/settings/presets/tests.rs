use std::path::Path;

use super::layer::Layer;
use super::{file, names, overlay};
use crate::settings::{Preferences, Preset};

/// Preferences with presets "Anime" (bold subtitles) and "Loud" (a 200% maximum).
fn with_presets() -> Preferences {
    let mut prefs = Preferences::default();
    let anime = toml::toml! { [subtitles] bold = true };
    let loud = toml::toml! { [audio] max_volume_percent = 200 };
    prefs.presets.list = vec![
        Preset { name: "Anime".into(), values: anime },
        Preset { name: "Loud".into(), values: loud },
    ];
    prefs
}

#[test]
fn selecting_a_preset_lays_its_values_over_the_defaults_and_back() {
    let mut prefs = with_presets();
    prefs.audio.max_volume_percent = 150;
    let mut layer = Layer::default();

    layer.select(&mut prefs, Some("Anime"));
    assert!(prefs.subtitles.bold);
    assert_eq!(prefs.audio.max_volume_percent, 150, "a default the preset doesn't change shows");
    assert_eq!(prefs.presets.active, "Anime");

    layer.select(&mut prefs, Some("Loud"));
    assert!(!prefs.subtitles.bold, "the last preset's values are gone");
    assert_eq!(prefs.audio.max_volume_percent, 200);

    layer.select(&mut prefs, None);
    assert_eq!(prefs.audio.max_volume_percent, 150);
    assert_eq!(prefs.presets.active, "");
}

#[test]
fn changes_go_to_the_preset_in_use_and_the_file_keeps_the_defaults() {
    let mut prefs = with_presets();
    let mut layer = Layer::default();
    layer.select(&mut prefs, Some("Anime"));

    let before = prefs.clone();
    prefs.video.picture.hue = 30.;
    prefs.subtitles.bold = false;
    layer.record(&before, &mut prefs);

    let values = overlay::flatten(&prefs.presets.find("Anime").expect("kept").values);
    assert!(values.contains_key("video.picture.hue"));
    assert!(!values.contains_key("subtitles.bold"), "a value back at its default is dropped");
    let on_disk = layer.for_disk(&prefs);
    assert!((on_disk.video.picture.hue - 0.).abs() < f32::EPSILON);
    assert_eq!(on_disk.presets.active, "Anime");

    layer.select(&mut prefs, None);
    assert!((prefs.video.picture.hue - 0.).abs() < f32::EPSILON);
}

#[test]
fn default_preset_changes_are_the_defaults() {
    let mut prefs = with_presets();
    let layer = Layer::default();
    let before = prefs.clone();
    prefs.subtitles.bold = true;
    layer.record(&before, &mut prefs);
    assert_eq!(prefs.presets, with_presets().presets);
    assert!(layer.for_disk(&prefs).subtitles.bold);
}

#[test]
fn loading_uses_the_saved_preset_or_falls_back_to_default() {
    let mut prefs = with_presets();
    prefs.presets.active = "Loud".into();
    Layer::load(&mut prefs);
    assert_eq!(prefs.audio.max_volume_percent, 200);

    let mut prefs = with_presets();
    prefs.presets.active = "Gone".into();
    Layer::load(&mut prefs);
    assert_eq!(prefs.presets.active, "");
}

#[test]
fn unknown_and_mismatched_values_are_kept_but_not_applied() {
    let mut prefs = Preferences::default();
    let values = toml::toml! {
        [audio]
        max_volume_percent = "loud"
        from_a_newer_version = 3
        [subtitles]
        bold = true
    };
    prefs.presets.list = vec![Preset { name: "Odd".into(), values: values.clone() }];
    let mut layer = Layer::default();
    layer.select(&mut prefs, Some("Odd"));
    assert_eq!(prefs.audio.max_volume_percent, Preferences::default().audio.max_volume_percent);
    assert!(prefs.subtitles.bold);

    let before = prefs.clone();
    prefs.subtitles.shadow = !prefs.subtitles.shadow;
    layer.record(&before, &mut prefs);
    let kept = &prefs.presets.list[0].values["audio"];
    assert_eq!(kept.get("from_a_newer_version"), values["audio"].get("from_a_newer_version"));
}

#[test]
fn deleting_the_preset_in_use_goes_back_to_default() {
    let mut prefs = with_presets();
    let mut layer = Layer::default();
    layer.select(&mut prefs, Some("Anime"));
    layer.delete(&mut prefs, "Anime");
    assert!(!prefs.subtitles.bold);
    assert_eq!(prefs.presets.active, "");
    assert!(prefs.presets.find("Anime").is_none());
}

#[test]
fn renaming_follows_the_preset_in_use_and_refuses_empty_names() {
    let mut prefs = with_presets();
    let mut layer = Layer::default();
    layer.select(&mut prefs, Some("Anime"));
    assert_eq!(Layer::rename(&mut prefs, "Anime", "  "), None);
    assert_eq!(Layer::rename(&mut prefs, "Anime", "loud").as_deref(), Some("loud (2)"));
    assert_eq!(prefs.presets.active, "loud (2)");
    assert_eq!(Layer::rename(&mut prefs, "loud (2)", "Cartoons").as_deref(), Some("Cartoons"));
}

#[test]
fn new_and_copied_presets_get_free_names() {
    let mut prefs = with_presets();
    let mut layer = Layer::default();
    assert_eq!(layer.create(&mut prefs), names::NEW);
    assert_eq!(layer.create(&mut prefs), "New Preset (2)");
    assert_eq!(prefs.presets.active, "New Preset (2)");
    assert_eq!(Layer::duplicate(&mut prefs, "Anime").as_deref(), Some("Anime (2)"));
    assert_eq!(Layer::duplicate(&mut prefs, "Anime (2)").as_deref(), Some("Anime (3)"));
    let order: Vec<_> = prefs.presets.list.iter().map(|preset| preset.name.as_str()).collect();
    assert_eq!(order[..4], ["Anime", "Anime (2)", "Anime (3)", "Loud"]);
}

#[test]
fn names_are_trimmed_shortened_and_never_default_presets() {
    assert_eq!(names::unique(&[], "  Anime ", None), "Anime");
    assert_eq!(names::unique(&[], "default preset", None), "default preset (2)");
    assert_eq!(names::unique(&[], &"x".repeat(100), None).chars().count(), names::MAX_CHARS);
}

#[test]
fn preset_files_round_trip_and_keep_only_media_values() {
    let preset = with_presets().presets.list.remove(0);
    let text = file::render(&preset);
    assert_eq!(file::parse(&text, Path::new("x.vlcpreset")), Ok(preset));

    let foreign = "vlc_preset = 1\n[values.network]\nhttp_proxy = \"evil\"\n";
    let parsed = file::parse(foreign, Path::new("/tmp/Shared.vlcpreset")).expect("a preset");
    assert_eq!(parsed.name, "Shared");
    assert!(parsed.values.is_empty());
    assert!(file::parse("title = 1", Path::new("a.toml")).is_err());
    assert_eq!(file::file_name("A/B: C"), "A-B- C.vlcpreset");
}
