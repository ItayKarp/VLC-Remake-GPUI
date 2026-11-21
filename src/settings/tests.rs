//! S0.1.10: round-trip, corrupt files, unknown keys and enum values, migration from
//! v0, awkward paths, and crash safety.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use super::paths::Locations;
use super::store::{Notice, SettingsState};
use super::{storage, stored_path};
use crate::theme::{Accent, ThemeMode};

/// A fresh directory per test, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "rvlc-settings-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    fn locations(&self) -> Locations {
        Locations::in_dir(&self.0)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn write(&self, name: &str, contents: &str) {
        fs::write(self.path(name), contents).unwrap();
    }

    fn read(&self, name: &str) -> String {
        fs::read_to_string(self.path(name)).unwrap()
    }

    fn load(&self) -> SettingsState {
        SettingsState::load(Some(self.locations()))
    }

    fn names(&self) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(&self.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn save(state: &mut SettingsState) {
    let job = state.prepare_save().expect("something to save");
    let generation = job.generation();
    let result = job.run();
    assert_eq!(result, Ok(()));
    state.finish_save(generation, result);
}

fn awkward_paths() -> Vec<PathBuf> {
    vec![
        PathBuf::from(r#"C:\Users\me\My "quoted" Videos"#),
        PathBuf::from(r"\\server\share\trailing\"),
        PathBuf::from("/home/me/line\nbreak/and\ttab"),
        PathBuf::from("/home/me/Видео/🎬 films/שלום"),
        PathBuf::from("/home/me/os-path:looks-encoded"),
        PathBuf::from("/home/me/'single' and \"double\" and \\back"),
    ]
}

#[test]
fn missing_files_load_defaults_without_notices() {
    let dir = TempDir::new();
    let mut state = dir.load();
    assert_eq!(state.settings, Default::default());
    assert!(state.take_notices().is_empty());
    assert!(!state.is_dirty());
    assert!(state.prepare_save().is_none());
}

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
    assert!(prefs.starts_with("schema_version = 1\n"), "{prefs}");
    assert!(!prefs.contains("media_folder"), "{prefs}");
    assert!(!prefs.contains("onboarding_complete"), "{prefs}");
    assert_eq!(dir.load().settings, state.settings);
}

#[test]
fn corrupt_file_is_moved_aside_and_defaults_used() {
    let dir = TempDir::new();
    // What the v0 writer produced for a folder containing a quote.
    let corrupt = "onboarding_complete = true\nmedia_folder = \"C:\\\\My \"Videos\"\"\n";
    dir.write("settings.toml", corrupt);
    let mut state = dir.load();
    assert_eq!(state.settings, Default::default());

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
    assert_eq!(state.settings, Default::default());
    assert!(state.take_notices().is_empty());
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
    // A directory where the settings file should be makes every write fail.
    fs::create_dir(dir.path("settings.toml")).unwrap();
    fs::write(dir.path("settings.toml").join("x"), "").unwrap();
    let mut state = SettingsState::load(Some(Locations {
        prefs: dir.path("settings.toml").join("x").join("settings.toml"),
        local: dir.path("local.toml"),
    }));
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

#[test]
fn config_dir_override() {
    let locations = Locations::resolve_with(Some("/portable/cfg".into())).unwrap();
    assert!(locations.prefs.ends_with("cfg/settings.toml"));
    assert!(locations.local.ends_with("cfg/local.toml"));
    // Empty means "not set".
    assert_ne!(Locations::resolve_with(Some("".into())), Some(locations));
}

#[test]
fn stored_path_round_trips() {
    for path in awkward_paths() {
        assert_eq!(stored_path::decode(&stored_path::encode(&path)), Some(path));
    }
    assert_eq!(stored_path::decode("os-path:garbage"), None);
}

#[cfg(windows)]
#[test]
fn non_unicode_windows_path_round_trips() {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    // "C:\a" followed by an unpaired surrogate.
    let path = PathBuf::from(OsString::from_wide(&[0x43, 0x3A, 0x5C, 0x61, 0xD800]));
    assert!(path.to_str().is_none());
    let encoded = stored_path::encode(&path);
    assert!(encoded.starts_with("os-path:wide:"));
    assert_eq!(stored_path::decode(&encoded), Some(path));
}

#[cfg(unix)]
#[test]
fn non_utf8_unix_path_round_trips() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    let path = PathBuf::from(OsString::from_vec(b"/tmp/\xff\xfe".to_vec()));
    let encoded = stored_path::encode(&path);
    assert!(encoded.starts_with("os-path:bytes:"));
    assert_eq!(stored_path::decode(&encoded), Some(path));
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

// Crash safety (acceptance criterion): kill a process mid-save 100 times; the file
// must never be empty or half-written. Spawns this test binary as the writer.
// Run with `cargo test kill_during_save -- --ignored`.

const CRASH_DIR_ENV: &str = "RVLC_TEST_CRASH_DIR";

fn crash_variant(n: u32) -> String {
    let filler = "x".repeat(64 * 1024);
    format!("generation = {n}\nfiller = \"{filler}\"\nend = true\n")
}

#[test]
#[ignore = "helper process for kill_during_save"]
fn crash_writer() {
    let Some(dir) = std::env::var_os(CRASH_DIR_ENV) else {
        return;
    };
    let path = Path::new(&dir).join("settings.toml");
    let mut n = 0;
    loop {
        storage::save(&path, &crash_variant(n)).unwrap();
        n += 1;
    }
}

#[test]
#[ignore = "slow: spawns 100 processes"]
fn kill_during_save() {
    use std::process::{Command, Stdio};
    use std::time::{Duration, SystemTime};

    let dir = TempDir::new();
    let path = dir.path("settings.toml");
    storage::save(&path, &crash_variant(0)).unwrap();
    let mut progressed = false;
    for round in 0..100 {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "settings::tests::crash_writer", "--ignored"])
            .env(CRASH_DIR_ENV, &dir.0)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let jitter =
            SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().subsec_nanos() % 40;
        std::thread::sleep(Duration::from_millis(20 + u64::from(jitter)));
        child.kill().unwrap();
        child.wait().unwrap();

        let text = fs::read_to_string(&path).unwrap();
        let doc: toml::Table = toml::from_str(&text)
            .unwrap_or_else(|err| panic!("round {round}: settings file is broken: {err}"));
        assert_eq!(doc.get("end"), Some(&toml::Value::Boolean(true)), "round {round}");
        progressed |= doc.get("generation").and_then(toml::Value::as_integer) != Some(0);
    }
    assert!(progressed, "the writer never got to write; the test proved nothing");
}
