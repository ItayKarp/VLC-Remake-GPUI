//! Presets on disk (S5.11): the file keeps the defaults and the presets beside them, and the
//! preset in use comes back with the next launch.

use super::support::{TempDir, save};

#[test]
fn the_file_keeps_the_defaults_while_a_preset_is_in_use() {
    let dir = TempDir::new();
    let mut state = dir.load();
    state.update(|s| s.prefs.audio.max_volume_percent = 150);
    let (name, _) = state.edit_presets(super::super::presets::Layer::create);
    state.update(|s| s.prefs.audio.max_volume_percent = 250);
    save(&mut state);

    let text = dir.read("settings.toml");
    assert!(text.contains("max_volume_percent = 150"), "{text}");
    assert!(text.contains("max_volume_percent = 250"), "the preset's value: {text}");

    let reloaded = dir.load();
    assert_eq!(reloaded.settings.prefs.presets.active, name);
    assert_eq!(reloaded.settings.prefs.audio.max_volume_percent, 250);

    let mut reloaded = reloaded;
    reloaded.edit_presets(|layer, prefs| layer.select(prefs, None));
    assert_eq!(reloaded.settings.prefs.audio.max_volume_percent, 150);
}
