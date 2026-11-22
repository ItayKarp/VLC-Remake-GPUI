//! Saving and reloading: round trips, when a save happens, what it preserves, and
//! what happens when it can't.

use std::fs;

use super::support::{TempDir, awkward_paths, save};
use crate::settings::paths::Locations;
use crate::settings::storage;
use crate::settings::store::{Notice, SettingsState};
use crate::theme::{Accent, ThemeMode};

#[test]
fn round_trip() {
    let dir = TempDir::new();
    let mut state = dir.load();
    state.update(|s| {
        s.prefs.theme = ThemeMode::Light;
        s.prefs.accent = Accent::Purple;
        s.local.onboarding_complete = true;
        s.local.media_folders = awkward_paths();
    });
    save(&mut state);

    let reloaded = dir.load();
    assert_eq!(reloaded.settings, state.settings);
    assert!(dir.read("settings.toml").starts_with("schema_version = 1\n"));
    assert!(dir.read("local.toml").contains("onboarding_complete = true"));
    // Machine-specific data stays out of the roaming file.
    assert!(!dir.read("settings.toml").contains("media_folders"));
}

#[test]
fn unchanged_update_does_not_save() {
    let dir = TempDir::new();
    let mut state = dir.load();
    assert!(!state.update(|s| s.prefs.theme = ThemeMode::default()));
    assert!(state.prepare_save().is_none());
}

#[test]
fn cancelled_save_is_retried_by_flush() {
    let dir = TempDir::new();
    let mut state = dir.load();
    state.update(|s| s.prefs.accent = Accent::Teal);
    // The debounced task prepared a job, then got cancelled before running it.
    drop(state.prepare_save());
    save(&mut state);
    assert_eq!(dir.load().settings.prefs.accent, Accent::Teal);
    assert!(state.prepare_save().is_none());
}

#[test]
fn unknown_keys_and_comments_survive_a_save() {
    let dir = TempDir::new();
    dir.write(
        "settings.toml",
        "# my settings, edited by hand\nschema_version = 1\n\
         theme = 'dark' # keep it dark\n\
         accent = \"magenta\"\n\
         future_flag = true\n\n\
         [player]\n# comment in a section\nfuture_speed = 1.5\n\n\
         [future_section]\nx = [1, 2]\n",
    );
    let mut state = dir.load();
    state.update(|s| s.prefs.accent = Accent::Green);
    save(&mut state);

    let text = dir.read("settings.toml");
    for expected in [
        "# my settings, edited by hand",
        "theme = 'dark' # keep it dark",
        "accent = \"green\"",
        "future_flag = true",
        "# comment in a section",
        "future_speed = 1.5",
        "[future_section]",
        "x = [1, 2]",
    ] {
        assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
    }
}

#[test]
fn save_keeps_previous_file_as_backup_and_leaves_no_temp_files() {
    let dir = TempDir::new();
    let path = dir.path("settings.toml");
    storage::save(&path, "a = 1\n").unwrap();
    storage::save(&path, "a = 2\n").unwrap();
    assert_eq!(dir.read("settings.toml"), "a = 2\n");
    assert_eq!(dir.read("settings.toml.bak"), "a = 1\n");
    assert_eq!(dir.names(), ["settings.toml", "settings.toml.bak", "settings.toml.lock"]);
}

#[test]
fn no_config_dir_runs_in_memory() {
    let mut state = SettingsState::load(None);
    assert!(!state.is_persistent());
    assert!(matches!(state.take_notices().as_slice(), [Notice::NotPersisted { .. }]));
    state.update(|s| s.prefs.accent = Accent::Red);
    assert!(state.prepare_save().is_none());
    assert_eq!(state.settings.prefs.accent, Accent::Red);
}

#[test]
fn failed_save_keeps_changes_and_reports_once() {
    let dir = TempDir::new();
    // The settings folder is missing when loading (a fresh install), then a file
    // takes its name, so every write fails. Reading through a file fails differently
    // on Unix and Windows; a missing folder reads the same everywhere.
    let mut state = SettingsState::load(Some(Locations {
        prefs: dir.path("config").join("settings.toml"),
        local: dir.path("local.toml"),
    }));
    fs::write(dir.path("config"), "").unwrap();
    for accent in [Accent::Red, Accent::Blue] {
        state.update(|s| s.prefs.accent = accent);
        let job = state.prepare_save().unwrap();
        let generation = job.generation();
        let result = job.run();
        assert!(result.is_err());
        state.finish_save(generation, result);
        assert!(state.is_dirty());
    }
    let notices = state.take_notices();
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert!(matches!(notices[0], Notice::SaveFailed { .. }));
    assert_eq!(state.settings.prefs.accent, Accent::Blue);
}

#[cfg(unix)]
#[test]
fn symlinked_file_is_written_through() {
    let dir = TempDir::new();
    fs::create_dir(dir.path("dotfiles")).unwrap();
    dir.write("dotfiles/settings.toml", "a = 1\n");
    std::os::unix::fs::symlink(dir.path("dotfiles/settings.toml"), dir.path("settings.toml"))
        .unwrap();
    storage::save(&dir.path("settings.toml"), "a = 2\n").unwrap();
    assert!(fs::symlink_metadata(dir.path("settings.toml")).unwrap().file_type().is_symlink());
    assert_eq!(dir.read("dotfiles/settings.toml"), "a = 2\n");
}

#[test]
fn out_of_range_values_are_clamped_on_load_and_saved() {
    let dir = TempDir::new();
    dir.write("settings.toml", "schema_version = 1\n[audio]\nmax_volume_percent = 999\n");
    let mut state = dir.load();
    assert_eq!(state.settings.prefs.audio.max_volume_percent, 300);
    assert!(state.is_dirty());
    save(&mut state);
    assert!(dir.read("settings.toml").contains("max_volume_percent = 300"));
}

#[test]
fn the_live_mirror_is_never_saved() {
    let dir = TempDir::new();
    let mut state = dir.load();
    assert!(state.update(|s| s.live.shuffle = true), "observers still hear of it");
    assert!(!state.is_dirty(), "nothing to write");
    state.update(|s| s.prefs.player.seek_precisely = false);
    save(&mut state);
    assert!(!dir.read("settings.toml").contains("shuffle"));
    assert!(!dir.load().settings.live.shuffle);
}
