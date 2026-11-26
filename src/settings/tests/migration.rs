//! Schema versions: migrating the v0 onboarding file, and leaving newer files alone.

use std::path::PathBuf;

use super::support::{TempDir, save};
use crate::theme::{Accent, ThemeMode};

#[test]
fn newer_schema_version_is_not_downgraded() {
    let dir = TempDir::new();
    dir.write("settings.toml", "schema_version = 7\naccent = \"red\"\n");
    let mut state = dir.load();
    assert_eq!(state.settings.prefs.accent, Accent::Red);
    state.update(|s| s.prefs.accent = Accent::Blue);
    save(&mut state);
    assert!(dir.read("settings.toml").contains("schema_version = 7"));
}

#[test]
fn migrates_the_v0_onboarding_file() {
    let dir = TempDir::new();
    // Exactly what the first onboarding build wrote.
    dir.write(
        "settings.toml",
        "onboarding_complete = true\nmedia_folder = \"C:\\\\Users\\\\me\\\\Videos\"\n\
         theme = \"system\"\naccent = \"pink\"\n",
    );
    let mut state = dir.load();
    assert!(state.is_dirty(), "a migrated file should be saved");
    assert_eq!(state.settings.prefs.theme, ThemeMode::System);
    assert_eq!(state.settings.prefs.accent, Accent::Pink);
    assert!(state.settings.local.onboarding_complete);
    assert_eq!(state.settings.local.media_folders, vec![PathBuf::from(r"C:\Users\me\Videos")]);

    save(&mut state);
    let prefs = dir.read("settings.toml");
    assert!(prefs.starts_with("schema_version = 2\n"), "{prefs}");
    assert!(!prefs.contains("media_folder"), "{prefs}");
    assert!(!prefs.contains("onboarding_complete"), "{prefs}");
    assert_eq!(dir.load().settings, state.settings);
}

#[test]
fn v1_files_take_the_opt_in_diagnostics_defaults() {
    let dir = TempDir::new();
    dir.write(
        "settings.toml",
        "schema_version = 1\naccent = \"pink\"\n\n[privacy]\nredact_file_names_in_logs = false\n\n\
         [diagnostics]\ncollect_statistics = true\n",
    );
    let state = dir.load();
    assert!(state.is_dirty(), "a migrated file should be saved");
    assert!(state.settings.prefs.privacy.redact_file_names_in_logs);
    assert!(!state.settings.prefs.diagnostics.collect_statistics);
    assert_eq!(state.settings.prefs.accent, Accent::Pink);
}
