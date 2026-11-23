//! Setting presets (S5.11): named overlays over the Playback, Video, Audio and Subtitles pages
//! ("Anime", "Loud and then Quiet Movies"), and Default Preset, the settings themselves.
//!
//! A preset keeps only the values it changes, so a default it doesn't change still shows
//! through. While a preset is in use, changing a setting on those pages, from Settings or a
//! menu, changes the preset; on Default Preset it changes the defaults. The settings read
//! everywhere are what's in effect, so nothing else knows about presets ([`layer`]).
//!
//! A player profile (S8.8, D12) is a player layout plus one of these presets, not a second
//! system (`settings::profiles`): renaming or deleting a preset renames or forgets it in the
//! profiles that use it.

pub mod file;
mod layer;
pub mod names;
mod overlay;
#[cfg(test)]
mod tests;

use gpui::{App, BorrowAppContext};

pub use layer::Layer;

use super::{Preset, SettingsStore, schedule_save};

/// The preset in use, or `None` for Default Preset.
pub fn active(cx: &App) -> Option<&str> {
    super::get(cx).prefs.presets.active().map(|preset| preset.name.as_str())
}

/// Uses the preset named `name`, or the defaults for `None`.
pub fn select(cx: &mut App, name: Option<&str>) {
    edit(cx, |layer, prefs| layer.select(prefs, name));
}

/// Adds an empty preset and uses it; answers its name.
pub fn create(cx: &mut App) -> String {
    edit(cx, Layer::create)
}

/// Adds `preset` (read from a file) and uses it; answers its name, numbered when taken.
pub fn add(cx: &mut App, preset: Preset) -> String {
    edit(cx, |layer, prefs| layer.add(prefs, preset))
}

/// Renames preset `from` to `to`; answers the name it got, or `None` when `to` is empty.
pub fn rename(cx: &mut App, from: &str, to: &str) -> Option<String> {
    edit(cx, |_, prefs| {
        let renamed = Layer::rename(prefs, from, to);
        if let Some(to) = &renamed {
            super::profiles::book::preset_renamed(prefs, from, to);
        }
        renamed
    })
}

/// Copies preset `name` under a numbered name; answers the copy's name.
pub fn duplicate(cx: &mut App, name: &str) -> Option<String> {
    edit(cx, |_, prefs| Layer::duplicate(prefs, name))
}

/// Deletes preset `name`, going back to Default Preset when it's in use.
pub fn delete(cx: &mut App, name: &str) {
    edit(cx, |layer, prefs| {
        layer.delete(prefs, name);
        super::profiles::book::preset_deleted(prefs, name);
    });
}

/// Runs `f` on the presets and saves when anything changed. Views observing the settings see
/// the change.
fn edit<R>(cx: &mut App, f: impl FnOnce(&mut Layer, &mut super::Preferences) -> R) -> R {
    let (result, changed) =
        cx.update_global::<SettingsStore, _>(|store, _| store.state.edit_presets(f));
    if changed {
        schedule_save(cx);
    }
    result
}
