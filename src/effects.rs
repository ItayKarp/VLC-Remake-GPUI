//! The effects architecture (S7.1): how the player's settings panel and the Settings pages share
//! the audio and video effects.
//!
//! **Layers (D13).** What plays is the Settings pages' values (the defaults, through the preset
//! in use, S5.11) with the panel's session values written over them ([`Session`]). Per-file
//! values exist only for the subtitle and audio delays, which belong to one media's track (S6.3,
//! S7.6) and live with the playback history, not here.
//!
//! **Session and defaults (D24).** A value the panel changes lasts until VLC quits, carried from
//! item to item, and is never saved; while it is held, changing its default in Settings doesn't
//! move it. Resetting a control or a section ([`reset`]) lets it follow its default again, and
//! Save as Default ([`save_as_default`]) writes a tab's held values into its Settings page, which
//! goes to the preset in use as any change there does.
//!
//! **Sections (D23).** The panel groups its controls in [`EffectSection`]s, each with a title
//! and an enable toggle; moving a control of a section that is off turns it on, so a change is
//! never invisible. The panel applies changes live, about 30 times a second while dragging
//! (`main_window/picture.rs`).

mod section;
mod session;

use gpui::{App, BorrowAppContext as _, Global};

pub use section::{EffectSection, VIDEO_COLORS, VIDEO_CROP, VIDEO_DEFAULTS, VIDEO_ESSENTIAL};
pub use session::Session;

use crate::settings::registry::{self, Value};
use crate::settings::{self, Settings};

/// The session layer, shared by every window; observe it as the settings are observed.
#[derive(Default)]
pub struct EffectSession(Session);

impl Global for EffectSession {}

/// The panel's held values; empty before it changed any.
fn session(cx: &App) -> Option<&Session> {
    cx.try_global::<EffectSession>().map(|global| &global.0)
}

fn update(cx: &mut App, f: impl FnOnce(&mut Session)) {
    cx.update_default_global::<EffectSession, _>(|global, _| f(&mut global.0));
}

/// The settings in effect: the defaults with the panel's held values over them.
pub fn effective(cx: &App) -> Settings {
    let defaults = settings::get(cx);
    session(cx).map_or_else(|| defaults.clone(), |session| session.overlay(defaults))
}

/// The value in effect for the setting `key`.
pub fn value(cx: &App, key: &str) -> Option<Value> {
    registry::find(key).map(|def| def.value(&effective(cx)))
}

/// The value the setting `key` has in Settings, which a reset goes back to.
pub fn default_value(cx: &App, key: &str) -> Option<Value> {
    registry::find(key).map(|def| def.value(settings::get(cx)))
}

/// Whether the panel holds a value for `key`.
pub fn is_changed(cx: &App, key: &str) -> bool {
    session(cx).is_some_and(|session| session.is_changed(key))
}

/// Whether the panel holds a value for any of `section`'s settings.
pub fn any_changed(cx: &App, section: &EffectSection) -> bool {
    section.keys().any(|key| is_changed(cx, key))
}

/// Whether the panel holds a value for any of the Video tab's framing: its Defaults rows or its
/// Crop section.
pub fn framing_changed(cx: &App) -> bool {
    VIDEO_DEFAULTS.iter().copied().chain(VIDEO_CROP.keys()).any(|key| is_changed(cx, key))
}

/// Sets `key`, a control outside any section, for the session.
pub fn set(cx: &mut App, key: &'static str, to: Value) {
    update(cx, |session| session.set(key, to));
}

/// Sets `section`'s toggle for the session.
pub fn set_enabled(cx: &mut App, section: &EffectSection, on: bool) {
    let key = section.toggle;
    update(cx, |session| session.set(key, Value::Bool(on)));
}

/// Sets one of `section`'s controls for the session, turning the section on if it was off; a
/// switch inside it turned off leaves the section as it is.
pub fn change(cx: &mut App, section: &EffectSection, key: &'static str, to: Value) {
    let off = to != Value::Bool(false) && value(cx, section.toggle) == Some(Value::Bool(false));
    update(cx, |session| {
        session.set(key, to);
        if off {
            session.set(section.toggle, Value::Bool(true));
        }
    });
}

/// Lets each of `keys` follow its default again (a control's or a section's reset).
pub fn reset<'a>(cx: &mut App, keys: impl IntoIterator<Item = &'a str>) {
    let keys: Vec<&str> = keys.into_iter().collect();
    if keys.iter().any(|key| is_changed(cx, key)) {
        update(cx, |session| {
            session.reset(keys);
        });
    }
}

/// Writes the held values of `keys` into their Settings pages, after which they follow those
/// pages again.
pub fn save_as_default<'a>(cx: &mut App, keys: impl IntoIterator<Item = &'a str>) {
    let mut taken = Vec::new();
    update(cx, |session| taken = session.take(keys));
    if !taken.is_empty() {
        settings::update(cx, |settings| session::write(settings, taken));
    }
}

#[cfg(test)]
mod tests {
    use gpui::TestAppContext;

    use super::*;

    const BRIGHTNESS: &str = "video.picture.brightness";

    fn brightness(cx: &App) -> Option<Value> {
        value(cx, BRIGHTNESS)
    }

    #[gpui::test]
    fn moving_a_control_turns_its_section_on(cx: &mut TestAppContext) {
        cx.update(|cx| {
            settings::init_in_memory(cx);
            change(cx, &VIDEO_ESSENTIAL, BRIGHTNESS, Value::Number(1.5));
            assert_eq!(value(cx, VIDEO_ESSENTIAL.toggle), Some(Value::Bool(true)));
            assert!(!settings::get(cx).prefs.video.picture.enabled, "the default stays off");
            assert!(any_changed(cx, &VIDEO_ESSENTIAL));
        });
    }

    #[gpui::test]
    fn the_session_wins_over_settings_until_reset(cx: &mut TestAppContext) {
        cx.update(|cx| {
            settings::init_in_memory(cx);
            change(cx, &VIDEO_ESSENTIAL, BRIGHTNESS, Value::Number(1.5));
            settings::update(cx, |s| s.prefs.video.picture.brightness = 0.5);
            assert_eq!(brightness(cx), Some(Value::Number(1.5)));
            reset(cx, [BRIGHTNESS]);
            assert_eq!(brightness(cx), Some(Value::Number(0.5)));
            settings::update(cx, |s| s.prefs.video.picture.brightness = 0.75);
            assert_eq!(brightness(cx), Some(Value::Number(0.75)), "it follows Settings again");
        });
    }

    #[gpui::test]
    fn a_framing_value_is_held_for_the_session(cx: &mut TestAppContext) {
        cx.update(|cx| {
            settings::init_in_memory(cx);
            assert!(!framing_changed(cx));
            set(cx, "video.rotation", Value::Choice("90"));
            assert!(framing_changed(cx));
            let video = effective(cx).prefs.video;
            assert_eq!(video.rotation, crate::engine::transform::Rotation::Quarter);
            assert_eq!(settings::get(cx).prefs.video.rotation.degrees(), 0, "never saved");
        });
    }

    #[gpui::test]
    fn save_as_default_writes_the_section_into_settings(cx: &mut TestAppContext) {
        cx.update(|cx| {
            settings::init_in_memory(cx);
            change(cx, &VIDEO_ESSENTIAL, BRIGHTNESS, Value::Number(1.5));
            save_as_default(cx, VIDEO_ESSENTIAL.keys());
            let picture = settings::get(cx).prefs.video.picture;
            assert!(picture.enabled);
            assert!((picture.brightness - 1.5).abs() < f32::EPSILON);
            assert!(!any_changed(cx, &VIDEO_ESSENTIAL));
            assert_eq!(default_value(cx, BRIGHTNESS), Some(Value::Number(1.5)));
        });
    }
}
