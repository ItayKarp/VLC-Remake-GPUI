//! Every command's keys once the user's changes (S5.6.3) are laid over the defaults:
//! [`Shortcuts`], which the Keyboard Shortcuts dialog edits and [`super::apply`] binds.
//!
//! Loading keeps what it can (S5.6 edge cases): an entry for a command that doesn't exist, a
//! second entry for the same command, a key that doesn't parse or the OS keeps, and a key
//! another command already has are each left out and reported.

use gpui::{Action, Keystroke, OperatingSystem};

use super::check::{self, same_key};
use super::commands::{COMMANDS, Command};
use crate::settings::{CustomBinding, KeymapSettings};

/// Each command's keys, in [`COMMANDS`] order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shortcuts {
    os: OperatingSystem,
    keys: Vec<Vec<Keystroke>>,
}

/// Another command that has a key already.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Conflict {
    /// The other command's index in [`COMMANDS`].
    pub command: usize,
}

impl Conflict {
    /// The other command's name, in the interface language.
    pub fn label(self) -> &'static str {
        COMMANDS[self.command].title()
    }
}

impl Shortcuts {
    /// Every command with its default keys on `os`.
    pub fn defaults(os: OperatingSystem) -> Self {
        let keys = COMMANDS.iter().map(|command| default_keys(command, os)).collect();
        Self { os, keys }
    }

    /// The defaults with `custom` laid over them, and what was left out, one line each.
    pub fn load(custom: &KeymapSettings, os: OperatingSystem) -> (Self, Vec<String>) {
        let mut shortcuts = Self::defaults(os);
        let mut problems = Vec::new();
        let mut changed = Vec::new();
        for entry in &custom.bindings {
            let Some(index) = COMMANDS.iter().position(|c| c.action.name() == entry.command) else {
                problems.push(format!("there's no command `{}`", entry.command));
                continue;
            };
            if changed.contains(&index) {
                problems.push(format!("`{}` is listed twice; the first is used", entry.command));
                continue;
            }
            changed.push(index);
            shortcuts.keys[index] = shortcuts.read_keys(index, &entry.keys, &mut problems);
        }
        shortcuts.drop_conflicts(&changed, &mut problems);
        (shortcuts, problems)
    }

    /// The OS the keys are for.
    pub fn os(&self) -> OperatingSystem {
        self.os
    }

    /// Command `index`'s keys, the primary one first.
    pub fn keys(&self, index: usize) -> &[Keystroke] {
        &self.keys[index]
    }

    /// Gives command `index` exactly `keys`.
    pub fn set(&mut self, index: usize, keys: Vec<Keystroke>) {
        self.keys[index] = keys;
    }

    /// Gives command `index` its default keys again.
    pub fn reset(&mut self, index: usize) {
        self.keys[index] = default_keys(&COMMANDS[index], self.os);
    }

    /// Whether command `index` has its default keys.
    pub fn is_default(&self, index: usize) -> bool {
        same_keys(&self.keys[index], &default_keys(&COMMANDS[index], self.os))
    }

    /// Whether any command's keys were changed.
    pub fn is_customized(&self) -> bool {
        (0..COMMANDS.len()).any(|index| !self.is_default(index))
    }

    /// Whether command `index` may have `keystroke`: any allowed key, and its own defaults.
    pub fn may_use(&self, index: usize, keystroke: &Keystroke) -> Result<(), check::Refusal> {
        let defaults = default_keys(&COMMANDS[index], self.os);
        if defaults.iter().any(|default| same_key(default, keystroke)) {
            return Ok(());
        }
        check::allowed(keystroke, self.os)
    }

    /// The command other than `index` that `keystroke` would also reach, if any.
    pub fn conflict(&self, index: usize, keystroke: &Keystroke) -> Option<Conflict> {
        let area = COMMANDS[index].area;
        let other = (0..COMMANDS.len()).find(|&other| {
            other != index
                && COMMANDS[other].area.overlaps(area)
                && self.keys[other].iter().any(|key| same_key(key, keystroke))
        });
        other.map(|command| Conflict { command })
    }

    /// Takes `keystroke` from whichever command had it (Replace).
    pub fn take_from_others(&mut self, index: usize, keystroke: &Keystroke) {
        while let Some(Conflict { command }) = self.conflict(index, keystroke) {
            self.keys[command].retain(|key| !same_key(key, keystroke));
        }
    }

    /// The changes from the defaults, as the settings store them.
    pub fn to_settings(&self) -> KeymapSettings {
        let changed = (0..COMMANDS.len()).filter(|&index| !self.is_default(index));
        let bindings = changed
            .map(|index| {
                let keys = self.keys[index].iter().map(|key| key.unparse(self.os)).collect();
                CustomBinding::new(COMMANDS[index].action.name(), keys)
            })
            .collect();
        KeymapSettings { bindings }
    }

    /// Each command with its keys, in [`COMMANDS`] order.
    pub fn iter(&self) -> impl Iterator<Item = (&'static Command, &[Keystroke])> {
        COMMANDS.iter().zip(self.keys.iter().map(Vec::as_slice))
    }

    /// The index of the command that dispatches `action`.
    pub fn index_of(action: &dyn Action) -> Option<usize> {
        COMMANDS.iter().position(|command| command.action.partial_eq(action))
    }

    /// Command `index`'s keys read from `sources`, leaving out (and reporting) what isn't
    /// allowed. Its own defaults are always allowed.
    fn read_keys(
        &self,
        index: usize,
        sources: &[String],
        problems: &mut Vec<String>,
    ) -> Vec<Keystroke> {
        let mut keys: Vec<Keystroke> = Vec::new();
        for source in sources {
            let keystroke = Keystroke::parse_with_os(source, self.os)
                .map_err(|_| check::Refusal::Invalid)
                .and_then(|key| self.may_use(index, &key).map(|()| key));
            match keystroke {
                Ok(key) if !keys.iter().any(|k| same_key(k, &key)) => keys.push(key),
                Ok(_) => {}
                Err(refusal) => problems.push(format!(
                    "`{source}` for {}: {}",
                    COMMANDS[index].label,
                    refusal.message()
                )),
            }
        }
        keys
    }

    /// Leaves out the changed commands' keys that another command already has.
    fn drop_conflicts(&mut self, changed: &[usize], problems: &mut Vec<String>) {
        for &index in changed {
            let keys = std::mem::take(&mut self.keys[index]);
            for key in keys {
                match self.conflict(index, &key) {
                    Some(conflict) => problems.push(format!(
                        "`{}` for {} is already used by {}",
                        key.unparse(self.os),
                        COMMANDS[index].label,
                        conflict.label()
                    )),
                    None => self.keys[index].push(key),
                }
            }
        }
    }
}

/// `command`'s default keys on `os`; the table's keys always parse (a test checks).
fn default_keys(command: &Command, os: OperatingSystem) -> Vec<Keystroke> {
    let sources = command.keys.for_os(os == OperatingSystem::Mac);
    sources.iter().filter_map(|source| Keystroke::parse_with_os(source, os).ok()).collect()
}

/// Whether `a` and `b` are the same keys in the same order.
fn same_keys(a: &[Keystroke], b: &[Keystroke]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same_key(a, b))
}

#[cfg(test)]
mod tests;
