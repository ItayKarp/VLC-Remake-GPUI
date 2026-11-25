use std::fs;

use super::*;
use crate::opening::scratch::scratch_dir;
use crate::updates::launcher::tests::Recorder;
use crate::updates::state::Backup;

fn running() -> Version {
    Version::new(0, 2, 0)
}

fn trial(version: Version, starts: u32) -> Trial {
    let backup = Backup::Msi { installed: "new.msi".into(), previous: None };
    Trial { version, previous: Version::new(0, 1, 0), starts, backup }
}

/// A folder holding `trial` in its state file.
fn on_trial(trial: Trial) -> PathBuf {
    let dir = scratch_dir();
    UpdateState { trial: Some(trial), ..UpdateState::default() }.save(&dir);
    dir
}

/// An AppImage at `dir` updated to "new", the previous one "old" kept beside it.
fn moved_appimage(dir: &Path) -> Backup {
    let installed = dir.join("VLC.AppImage");
    let saved = dir.join(".VLC.AppImage.previous");
    fs::write(&installed, "new").unwrap();
    fs::write(&saved, "old").unwrap();
    Backup::Moved { installed, saved }
}

/// Launches the app at the running version as a standalone instance.
fn launch(dir: &Path, launcher: &Recorder) -> Option<Role> {
    launched_in(Role::Standalone, dir, &running(), launcher)
}

#[test]
fn the_third_launch_without_a_start_rolls_back() {
    let running = running();
    assert_eq!(decide(None, &running), Decision::Nothing);
    assert_eq!(decide(Some(&trial(running.clone(), 0)), &running), Decision::Count);
    assert_eq!(decide(Some(&trial(running.clone(), 1)), &running), Decision::Count);
    assert_eq!(decide(Some(&trial(running.clone(), 2)), &running), Decision::RollBack);
}

#[test]
fn another_version_ends_the_trial() {
    let other = trial(Version::new(0, 3, 0), 2);
    assert_eq!(decide(Some(&other), &running()), Decision::Forget);
}

#[test]
fn each_launch_counts_a_start_until_the_previous_version_comes_back() {
    let dir = scratch_dir();
    let backup = moved_appimage(&dir);
    let Backup::Moved { installed, .. } = &backup else { unreachable!("an AppImage is moved") };
    let dir = on_trial(Trial { backup: backup.clone(), ..trial(running(), 0) });
    let launcher = Recorder::default();
    for starts in 1..=MAX_FAILED_STARTS {
        assert!(matches!(launch(&dir, &launcher), Some(Role::Standalone)));
        assert_eq!(UpdateState::load(&dir).trial.map(|trial| trial.starts), Some(starts));
    }

    assert!(launch(&dir, &launcher).is_none(), "this process makes way");
    assert_eq!(fs::read_to_string(installed).unwrap(), "old");
    let state = UpdateState::load(&dir);
    assert_eq!(state.trial, None);
    assert_eq!(state.skipped, Some(running()));
    assert_eq!(state.rolled_back, Some(RolledBack { version: running() }));
    let program = installed.to_string_lossy().into_owned();
    assert_eq!(launcher.launched(), [[program]]);
}

#[test]
fn a_rolled_back_bundle_starts_from_where_this_program_ran() {
    let dir = scratch_dir();
    let (installed, saved) = (dir.join("VLC.app"), dir.join(".VLC.app.previous"));
    fs::create_dir_all(installed.join("Contents/New")).unwrap();
    fs::create_dir_all(saved.join("Contents/Old")).unwrap();
    let dir = on_trial(Trial {
        backup: Backup::Moved { installed: installed.clone(), saved },
        ..trial(running(), 2)
    });
    let launcher = Recorder::default();

    assert!(launch(&dir, &launcher).is_none());
    assert!(installed.join("Contents/Old").is_dir());
    let exe = std::env::current_exe().unwrap().to_string_lossy().into_owned();
    assert_eq!(launcher.launched(), [[exe]]);
}

#[test]
fn a_previous_version_that_fails_to_start_still_ends_this_process() {
    let dir = scratch_dir();
    let backup = moved_appimage(&dir);
    let dir = on_trial(Trial { backup, ..trial(running(), 2) });
    let launcher = Recorder::failing();
    assert!(launch(&dir, &launcher).is_none());
    assert_eq!(launcher.launched().len(), 1, "it tried to start it");
}

#[test]
fn a_failed_roll_back_ends_the_trial_and_keeps_running() {
    let dir = on_trial(trial(running(), 2));
    let launcher = Recorder::default();
    assert!(matches!(launch(&dir, &launcher), Some(Role::Standalone)));
    let state = UpdateState::load(&dir);
    assert_eq!(state.trial, None);
    assert_eq!(state.skipped, None);
    assert_eq!(state.rolled_back, None);
    assert_eq!(launcher.launched(), Vec::<Vec<String>>::new());
}

#[test]
fn a_version_installed_by_hand_ends_the_trial() {
    let dir = on_trial(trial(Version::new(0, 3, 0), 2));
    assert!(matches!(launch(&dir, &Recorder::default()), Some(Role::Standalone)));
    assert_eq!(UpdateState::load(&dir).trial, None);
}

#[test]
fn a_launch_forwarded_to_the_running_app_counts_nothing() {
    let dir = on_trial(trial(running(), 2));
    for role in [Role::Forwarded, Role::NotResponding] {
        let answered = launched_in(role, &dir, &running(), &Recorder::default());
        assert!(matches!(answered, Some(Role::Forwarded | Role::NotResponding)));
    }
    assert_eq!(UpdateState::load(&dir).trial.map(|trial| trial.starts), Some(2));
    assert!(matches!(launched(Role::Forwarded), Some(Role::Forwarded)));
}

#[test]
fn without_a_trial_a_launch_writes_nothing() {
    let dir = scratch_dir();
    assert!(matches!(launch(&dir, &Recorder::default()), Some(Role::Standalone)));
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
}

#[test]
fn a_confirmed_start_throws_the_previous_version_away() {
    let dir = scratch_dir();
    let backup = moved_appimage(&dir);
    let Backup::Moved { installed, saved } = &backup else { unreachable!("an AppImage is moved") };
    let (installed, saved) = (installed.clone(), saved.clone());
    let dir = on_trial(Trial { backup, ..trial(running(), 1) });

    confirm_in(&dir, &running());
    assert!(!saved.exists());
    assert_eq!(fs::read_to_string(installed).unwrap(), "new");
    assert_eq!(UpdateState::load(&dir).trial, None);
}

#[test]
fn confirming_another_version_keeps_the_trial() {
    let kept = trial(Version::new(0, 3, 0), 1);
    let dir = on_trial(kept.clone());
    confirm_in(&dir, &running());
    assert_eq!(UpdateState::load(&dir).trial, Some(kept));
}
