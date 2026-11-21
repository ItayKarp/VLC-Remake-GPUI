//! Loading what's on disk when it isn't what we wrote: missing, empty or corrupt
//! files, and values this version doesn't understand.

use std::fs;
use std::path::PathBuf;

use super::support::TempDir;
use crate::settings::Settings;
use crate::settings::store::Notice;
use crate::theme::{Accent, ThemeMode};

#[test]
fn missing_files_load_defaults_without_notices() {
    let dir = TempDir::new();
    let mut state = dir.load();
    assert_eq!(state.settings, Settings::default());
    assert_eq!(state.take_notices(), Vec::<Notice>::new());
    assert!(!state.is_dirty());
    assert!(state.prepare_save().is_none());
}

#[test]
fn unknown_enum_value_falls_back_per_field() {
    let dir = TempDir::new();
    dir.write("settings.toml", "schema_version = 1\ntheme = \"light\"\naccent = \"magenta\"\n");
    dir.write(
        "local.toml",
        "onboarding_complete = \"yes\"\nmedia_folders = [\"/a\", 42, \"/b\"]\nwindow = 5\n",
    );
    let state = dir.load();
    assert_eq!(state.settings.prefs.theme, ThemeMode::Light);
    assert_eq!(state.settings.prefs.accent, Accent::default());
    assert!(!state.settings.local.onboarding_complete);
    assert_eq!(state.settings.local.media_folders, vec![PathBuf::from("/a"), PathBuf::from("/b")]);
}

#[test]
fn corrupt_file_is_moved_aside_and_defaults_used() {
    let dir = TempDir::new();
    // What the v0 writer produced for a folder containing a quote.
    let corrupt = "onboarding_complete = true\nmedia_folder = \"C:\\\\My \"Videos\"\"\n";
    dir.write("settings.toml", corrupt);
    let mut state = dir.load();
    assert_eq!(state.settings, Settings::default());

    let notices = state.take_notices();
    let [Notice::Corrupt { moved_to: Some(moved_to), restored_from_backup: false, .. }] =
        notices.as_slice()
    else {
        panic!("unexpected notices: {notices:?}");
    };
    assert_eq!(fs::read_to_string(moved_to).unwrap(), corrupt);
    assert!(!dir.path("settings.toml").exists());
    assert!(moved_to.file_name().unwrap().to_string_lossy().starts_with("settings.corrupt-"));
}

#[test]
fn corrupt_file_restores_from_backup() {
    let dir = TempDir::new();
    dir.write("settings.toml", "accent = \n");
    dir.write("settings.toml.bak", "schema_version = 1\naccent = \"yellow\"\n");
    let mut state = dir.load();
    assert_eq!(state.settings.prefs.accent, Accent::Yellow);
    assert!(matches!(
        state.take_notices().as_slice(),
        [Notice::Corrupt { restored_from_backup: true, .. }]
    ));
}

#[test]
fn empty_file_restores_from_backup() {
    let dir = TempDir::new();
    dir.write("settings.toml", "");
    dir.write("settings.toml.bak", "schema_version = 1\naccent = \"gray\"\n");
    let mut state = dir.load();
    assert_eq!(state.settings.prefs.accent, Accent::Gray);
    assert!(matches!(state.take_notices().as_slice(), [Notice::RestoredFromBackup { .. }]));
}

#[test]
fn empty_file_without_backup_is_treated_as_missing() {
    let dir = TempDir::new();
    dir.write("settings.toml", "");
    let mut state = dir.load();
    assert_eq!(state.settings, Settings::default());
    assert_eq!(state.take_notices(), Vec::<Notice>::new());
}
