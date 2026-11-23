//! The profiles over the preferences, without GPUI: switching, and the menu's own edits (new,
//! rename, delete, automatic switching).
//!
//! The profile in use keeps its layout and preset where everything reads them
//! (`prefs.player_layout`, `prefs.presets.active`); [`capture`] copies them into its entry before
//! anything reads or replaces the list, so the entry is never older than what's shown.

use std::borrow::Cow;

use crate::settings::presets::{Layer, names};
use crate::settings::{Automatic, Preferences, Profile};

/// The profiles a user starts with, in menu order: the designed four, the first in use.
pub const DESIGNED: [&str; 4] = ["Movies", "Shows", "Music", "Work"];
/// The name a new profile is offered.
pub const NEW: &str = "New Profile";

/// The profiles: the designed four while none was changed.
pub fn list(prefs: &Preferences) -> Cow<'_, [Profile]> {
    if prefs.profiles.list.is_empty() {
        Cow::Owned(designed())
    } else {
        Cow::Borrowed(&prefs.profiles.list)
    }
}

/// The name of the profile in use: the stored one if it exists, else the first.
pub fn active(prefs: &Preferences) -> String {
    let list = list(prefs);
    let stored = list.iter().find(|profile| profile.name == prefs.profiles.active);
    stored.or(list.first()).map_or_else(String::new, |profile| profile.name.clone())
}

/// Switches to profile `name`: its layout shows and its preset is put in use. Answers whether
/// there is such a profile.
pub fn select(layer: &mut Layer, prefs: &mut Preferences, name: &str) -> bool {
    capture(prefs);
    let Some(profile) = prefs.profiles.list.iter().find(|profile| profile.name == name).cloned()
    else {
        return false;
    };
    apply(layer, prefs, profile);
    true
}

/// Adds a profile named `wanted` (numbered when taken) holding the layout and preset in use, and
/// switches to it. Answers its name.
pub fn create(layer: &mut Layer, prefs: &mut Preferences, wanted: &str) -> String {
    capture(prefs);
    let name = unique(&prefs.profiles.list, wanted, None);
    let current = prefs.profiles.list.iter().find(|profile| profile.name == prefs.profiles.active);
    let profile = Profile {
        name: name.clone(),
        automatic: Automatic::Never,
        ..current.cloned().unwrap_or_default()
    };
    prefs.profiles.list.push(profile.clone());
    apply(layer, prefs, profile);
    name
}

/// Renames profile `from` to `to`, trimmed and numbered when taken. Answers the new name, or
/// `None` when `to` is empty or there's no such profile.
pub fn rename(prefs: &mut Preferences, from: &str, to: &str) -> Option<String> {
    capture(prefs);
    let index = prefs.profiles.list.iter().position(|profile| profile.name == from)?;
    if to.trim().is_empty() {
        return None;
    }
    let name = unique(&prefs.profiles.list, to, Some(index));
    if prefs.profiles.active == from {
        prefs.profiles.active.clone_from(&name);
    }
    prefs.profiles.list[index].name.clone_from(&name);
    Some(name)
}

/// Deletes profile `name`, switching to the one before it (or the next) when it's in use.
/// Answers whether it was deleted: the last profile stays.
pub fn delete(layer: &mut Layer, prefs: &mut Preferences, name: &str) -> bool {
    capture(prefs);
    let list = &prefs.profiles.list;
    let Some(index) = list.iter().position(|profile| profile.name == name) else { return false };
    if list.len() < 2 {
        return false;
    }
    if prefs.profiles.active == name {
        let other = list[index.checked_sub(1).unwrap_or(1)].clone();
        apply(layer, prefs, other);
    }
    prefs.profiles.list.remove(index);
    true
}

/// Makes `kind` switch to profile `name` by itself, taking it from any other profile; `Never`
/// stops `name` being switched to.
pub fn set_automatic(prefs: &mut Preferences, name: &str, kind: Automatic) {
    capture(prefs);
    for profile in &mut prefs.profiles.list {
        if profile.name == name {
            profile.automatic = kind;
        } else if kind != Automatic::Never && profile.automatic == kind {
            profile.automatic = Automatic::Never;
        }
    }
}

/// The profile `kind` of media switches to, if one is set to.
pub fn for_media(prefs: &Preferences, kind: Automatic) -> Option<String> {
    if kind == Automatic::Never {
        return None;
    }
    let list = list(prefs);
    list.iter().find(|profile| profile.automatic == kind).map(|profile| profile.name.clone())
}

/// Profiles using preset `from` use it as `to` after it was renamed.
pub fn preset_renamed(prefs: &mut Preferences, from: &str, to: &str) {
    for profile in prefs.profiles.list.iter_mut().filter(|profile| profile.preset == from) {
        to.clone_into(&mut profile.preset);
    }
}

/// Profiles using preset `name` use Default Preset after it was deleted.
pub fn preset_deleted(prefs: &mut Preferences, name: &str) {
    for profile in prefs.profiles.list.iter_mut().filter(|profile| profile.preset == name) {
        profile.preset.clear();
    }
}

/// Makes the list real (the designed four, before any change) and copies the layout and preset
/// in use into the profile in use.
fn capture(prefs: &mut Preferences) {
    if prefs.profiles.list.is_empty() {
        prefs.profiles.list = designed();
    }
    let active = active(prefs);
    prefs.profiles.active.clone_from(&active);
    let Some(profile) = prefs.profiles.list.iter_mut().find(|profile| profile.name == active)
    else {
        return;
    };
    profile.layout.clone_from(&prefs.player_layout);
    profile.preset.clone_from(&prefs.presets.active);
}

/// Shows `profile`'s layout and puts its preset in use; a preset deleted since falls back to
/// Default Preset.
fn apply(layer: &mut Layer, prefs: &mut Preferences, profile: Profile) {
    prefs.player_layout = profile.layout;
    layer.select(prefs, Some(profile.preset.as_str()).filter(|preset| !preset.is_empty()));
    prefs.profiles.active = profile.name;
}

fn designed() -> Vec<Profile> {
    DESIGNED.map(|name| Profile { name: name.to_owned(), ..Profile::default() }).to_vec()
}

/// `wanted`, trimmed, shortened and numbered as preset names are, among `list` but `except`.
fn unique(list: &[Profile], wanted: &str, except: Option<usize>) -> String {
    let others = list.iter().enumerate().filter(|(index, _)| Some(*index) != except);
    let others: Vec<&str> = others.map(|(_, profile)| profile.name.as_str()).collect();
    names::free(wanted, NEW, others.len(), |name| others.iter().any(|o| names::same(o, name)))
}
