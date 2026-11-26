//! Player profiles (S8.8): Movies, Shows, Music, Work and the user's own, each a player layout
//! (S8.7) and a setting preset (S5.11), as D12 decided: one system, not two.
//!
//! Switching profile shows its layout and puts its preset in use at once, during playback too.
//! While a profile is in use, Customize Player and the preset picker change it: its layout and
//! preset are the ones everything reads, copied into its entry when another is chosen
//! ([`book`]). A profile can be switched to by itself when music or video plays (S8.8.3).

pub mod book;
#[cfg(test)]
mod tests;

use gpui::{App, BorrowAppContext};

pub use book::NEW;

use super::{Automatic, Preferences, Profile, SettingsStore, schedule_save};
use crate::settings::presets::Layer;

/// The profiles, in menu order.
pub fn list(cx: &App) -> Vec<Profile> {
    book::list(&super::get(cx).prefs).into_owned()
}

/// The name of the profile in use.
pub fn active(cx: &App) -> String {
    book::active(&super::get(cx).prefs)
}

/// Switches to profile `name`.
pub fn select(cx: &mut App, name: &str) {
    edit(cx, |layer, prefs| book::select(layer, prefs, name));
}

/// Adds a profile named `wanted` holding the layout and preset in use, and switches to it;
/// answers its name.
pub fn create(cx: &mut App, wanted: &str) -> String {
    edit(cx, |layer, prefs| book::create(layer, prefs, wanted))
}

/// Renames profile `from` to `to`; answers the name it got, or `None` when `to` is empty.
pub fn rename(cx: &mut App, from: &str, to: &str) -> Option<String> {
    edit(cx, |_, prefs| book::rename(prefs, from, to))
}

/// Deletes profile `name`, switching away from it when it's in use; answers whether it was
/// deleted, which the last profile never is.
pub fn delete(cx: &mut App, name: &str) -> bool {
    edit(cx, |layer, prefs| book::delete(layer, prefs, name))
}

/// Makes `kind` of media switch to profile `name` by itself, or stops it with `Never`.
pub fn set_automatic(cx: &mut App, name: &str, kind: Automatic) {
    edit(cx, |_, prefs| book::set_automatic(prefs, name, kind));
}

/// The profile `kind` of media switches to, if one is set to.
pub fn for_media(cx: &App, kind: Automatic) -> Option<String> {
    book::for_media(&super::get(cx).prefs, kind)
}

/// Runs `f` on the profiles and the preset layer, and saves when anything changed. Views
/// observing the settings see the change.
fn edit<R>(cx: &mut App, f: impl FnOnce(&mut Layer, &mut Preferences) -> R) -> R {
    let (result, changed) =
        cx.update_global::<SettingsStore, _>(|store, _| store.state.edit_presets(f));
    if changed {
        schedule_save(cx);
    }
    result
}
