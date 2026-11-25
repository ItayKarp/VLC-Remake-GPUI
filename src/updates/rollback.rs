//! Rolling back an update that fails to start twice (S9.5.2).
//!
//! The update installed at quit is on trial: each launch of it counts a start before anything
//! else runs, and a start that lasts [`super::STARTED_AFTER`] (or ends in a clean quit) ends the
//! trial and throws the previous version away. A third launch that finds two starts counted
//! without that puts the previous version back, starts it and exits; the failed version is then
//! skipped, so it isn't offered again, and the previous version says what happened once.
//!
//! An update that can't run any code at all (a missing library) never counts a start, so it
//! can't be rolled back this way.

use std::path::PathBuf;
use std::process::Command;

use semver::Version;

use super::install;
use super::manifest::running_version;
use super::state::{self, RolledBack, Trial, UpdateState};
use crate::app::instance::Role;

/// How many launches may fail to start before the previous version comes back.
const MAX_FAILED_STARTS: u32 = 2;

/// What a launch does about the update on trial.
#[derive(Debug, PartialEq, Eq)]
enum Decision {
    /// Nothing is on trial.
    Nothing,
    /// Something else than the update runs (installed by hand): the trial is over.
    Forget,
    /// The update starts once more: count it.
    Count,
    /// The update failed to start too often: put the previous version back.
    RollBack,
}

fn decide(trial: Option<&Trial>, running: &Version) -> Decision {
    match trial {
        None => Decision::Nothing,
        Some(trial) if trial.version != *running => Decision::Forget,
        Some(trial) if trial.starts >= MAX_FAILED_STARTS => Decision::RollBack,
        Some(_) => Decision::Count,
    }
}

/// Follows the update on trial as this launch, as `role`, starts. Answers `None` when it put the
/// previous version back and started it, so this process exits.
pub fn launched(role: Role) -> Option<Role> {
    // Only a launch that runs the app counts: one forwarded to the running app started nothing.
    if matches!(role, Role::Forwarded | Role::NotResponding) {
        return Some(role);
    }
    let Some(Restart { program }) = follow_trial() else { return Some(role) };
    // The previous version must find the instance lock free.
    drop(role);
    if let Some(program) = program
        && let Err(error) = Command::new(&program).spawn()
    {
        tracing::error!("updates: couldn't start {}: {error}", program.display());
    }
    None
}

/// The previous version was put back, and this process makes way for it.
#[derive(Debug, PartialEq, Eq)]
struct Restart {
    /// The program to start it with; `None` when it starts on its own.
    program: Option<PathBuf>,
}

/// Counts this start of the update on trial, or rolls it back. Answers how the previous version
/// starts when it was rolled back.
fn follow_trial() -> Option<Restart> {
    let dir = state::data_dir()?;
    let mut state = UpdateState::load(&dir);
    let running = running_version();
    let trial = state.trial.as_mut()?;
    match decide(Some(trial), &running) {
        Decision::Nothing => return None,
        Decision::Forget => state.trial = None,
        Decision::Count => trial.starts += 1,
        Decision::RollBack => return roll_back(&mut state, &dir),
    }
    state.save(&dir);
    None
}

/// Puts the previous version back. Answers how it starts when it did; `None` when it couldn't,
/// so the update runs on.
fn roll_back(state: &mut UpdateState, dir: &std::path::Path) -> Option<Restart> {
    let trial = state.trial.take()?;
    let restored = install::restore(&trial.backup).inspect_err(|error| failed(&trial, error));
    if restored.is_ok() {
        rolled_back(&trial);
        state.skipped = state.skipped.take().max(Some(trial.version.clone()));
        state.rolled_back = Some(RolledBack { version: trial.version });
    }
    state.save(dir);
    restored.ok().map(|program| Restart { program })
}

fn rolled_back(trial: &Trial) {
    tracing::warn!("updates: {} didn't start twice; {} is back", trial.version, trial.previous);
}

fn failed(trial: &Trial, error: &std::io::Error) {
    tracing::error!("updates: couldn't put {} back: {error}", trial.previous);
}

/// The update on trial started: the trial is over and the previous version isn't needed.
pub fn confirm() {
    let Some(dir) = state::data_dir() else { return };
    let mut state = UpdateState::load(&dir);
    let running = running_version();
    if state.trial.as_ref().is_none_or(|trial| trial.version != running) {
        return;
    }
    if let Some(trial) = state.trial.take() {
        install::discard(&trial.backup);
        tracing::info!("updates: {} started; {} is gone", trial.version, trial.previous);
    }
    state.save(&dir);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updates::state::Backup;

    fn trial(version: Version, starts: u32) -> Trial {
        Trial {
            version,
            previous: Version::new(0, 1, 0),
            starts,
            backup: Backup::Msi { installed: "new.msi".into(), previous: None },
        }
    }

    #[test]
    fn the_third_launch_without_a_start_rolls_back() {
        let running = Version::new(0, 2, 0);
        assert_eq!(decide(None, &running), Decision::Nothing);
        assert_eq!(decide(Some(&trial(running.clone(), 0)), &running), Decision::Count);
        assert_eq!(decide(Some(&trial(running.clone(), 1)), &running), Decision::Count);
        assert_eq!(decide(Some(&trial(running.clone(), 2)), &running), Decision::RollBack);
    }

    #[test]
    fn another_version_ends_the_trial() {
        let other = trial(Version::new(0, 3, 0), 2);
        assert_eq!(decide(Some(&other), &Version::new(0, 2, 0)), Decision::Forget);
    }

    #[test]
    fn a_failed_roll_back_ends_the_trial_and_keeps_running() {
        let dir = std::env::temp_dir().join(format!("rvlc-rollback-{}", std::process::id()));
        let mut state =
            UpdateState { trial: Some(trial(Version::new(0, 2, 0), 2)), ..UpdateState::default() };
        assert_eq!(roll_back(&mut state, &dir), None);
        assert_eq!(state.trial, None);
        assert_eq!(state.skipped, None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
