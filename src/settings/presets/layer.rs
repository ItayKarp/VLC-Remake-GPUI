//! The preset in use over the defaults, without GPUI: switching, recording changes into the
//! preset in use, and the list's own edits (new, rename, duplicate, delete, import).
//!
//! The typed preferences always hold what's in effect, so nothing that reads a setting knows
//! about presets. While a custom preset is in use, [`Layer`] keeps the defaults it covered, puts
//! them back when another is chosen, and hands them to the save in place of the preset's values,
//! so the settings file holds the defaults and the presets beside them.

use std::borrow::Cow;

use super::names;
use super::overlay::{self, Leaves};
use crate::settings::{Preferences, Preset};

/// The defaults a custom preset covers while it's in use.
#[derive(Debug, Default)]
pub struct Layer {
    /// The preset sections' default values; `None` while Default Preset is in use.
    defaults: Option<Leaves>,
}

impl Layer {
    /// Puts the preset `prefs` names as in use over the defaults just read; a name no preset
    /// has falls back to Default Preset.
    pub fn load(prefs: &mut Preferences) -> Self {
        let mut layer = Self::default();
        let active = std::mem::take(&mut prefs.presets.active);
        layer.select(prefs, Some(active.as_str()).filter(|name| !name.is_empty()));
        layer
    }

    /// Uses the preset named `name`, or the defaults for `None`.
    pub fn select(&mut self, prefs: &mut Preferences, name: Option<&str>) {
        if let Some(defaults) = self.defaults.take() {
            overlay::apply(prefs, &defaults);
        }
        prefs.presets.active.clear();
        let Some(preset) = name.and_then(|name| prefs.presets.find(name)).cloned() else {
            return;
        };
        self.defaults = Some(overlay::leaves(prefs));
        overlay::apply(prefs, &overlay::flatten(&preset.values));
        prefs.presets.active = preset.name;
    }

    /// Records what changed from `before` to `after` into the preset in use: a value back at
    /// its default is dropped from the preset, any other is kept. Does nothing on Default
    /// Preset, whose changes are the defaults.
    pub fn record(&self, before: &Preferences, after: &mut Preferences) {
        let Some(defaults) = &self.defaults else { return };
        if overlay::same_sections(before, after) {
            return;
        }
        let (old, new) = (overlay::leaves(before), overlay::leaves(after));
        let active = after.presets.active.clone();
        let Some(preset) = after.presets.list.iter_mut().find(|preset| preset.name == active)
        else {
            return;
        };
        let mut values = overlay::flatten(&preset.values);
        for (path, value) in new.iter().filter(|(path, value)| old.get(*path) != Some(value)) {
            if defaults.get(path) == Some(value) {
                values.remove(path);
            } else {
                values.insert(path.clone(), value.clone());
            }
        }
        preset.values = overlay::nest(&values);
    }

    /// The preferences as the settings file keeps them: the defaults, with the preset in use
    /// named.
    pub fn for_disk<'a>(&self, prefs: &'a Preferences) -> Cow<'a, Preferences> {
        let Some(defaults) = &self.defaults else { return Cow::Borrowed(prefs) };
        let mut on_disk = prefs.clone();
        overlay::apply(&mut on_disk, defaults);
        Cow::Owned(on_disk)
    }

    /// Adds an empty preset, which shows the defaults until changed, and uses it. Answers its
    /// name.
    pub fn create(&mut self, prefs: &mut Preferences) -> String {
        self.add(prefs, Preset { name: names::NEW.to_owned(), values: toml::Table::new() })
    }

    /// Adds `preset`, numbered when its name is taken, and uses it. Answers its name.
    pub fn add(&mut self, prefs: &mut Preferences, mut preset: Preset) -> String {
        preset.name = names::unique(&prefs.presets.list, &preset.name, None);
        let name = preset.name.clone();
        prefs.presets.list.push(preset);
        self.select(prefs, Some(&name));
        name
    }

    /// Renames preset `from` to `to`, trimmed and numbered when taken. Answers the new name, or
    /// `None` when `to` is empty or there's no such preset.
    pub fn rename(prefs: &mut Preferences, from: &str, to: &str) -> Option<String> {
        let index = prefs.presets.list.iter().position(|preset| preset.name == from)?;
        if to.trim().is_empty() {
            return None;
        }
        let name = names::unique(&prefs.presets.list, to, Some(index));
        if prefs.presets.active == from {
            prefs.presets.active.clone_from(&name);
        }
        prefs.presets.list[index].name.clone_from(&name);
        Some(name)
    }

    /// Copies preset `name` under a numbered name, just after it. Answers the copy's name.
    pub fn duplicate(prefs: &mut Preferences, name: &str) -> Option<String> {
        let index = prefs.presets.list.iter().position(|preset| preset.name == name)?;
        let mut copy = prefs.presets.list[index].clone();
        copy.name = names::unique(&prefs.presets.list, name, None);
        let copied = copy.name.clone();
        prefs.presets.list.insert(index + 1, copy);
        Some(copied)
    }

    /// Deletes preset `name`, going back to Default Preset first when it's in use.
    pub fn delete(&mut self, prefs: &mut Preferences, name: &str) {
        if prefs.presets.active == name {
            self.select(prefs, None);
        }
        prefs.presets.list.retain(|preset| preset.name != name);
    }
}
