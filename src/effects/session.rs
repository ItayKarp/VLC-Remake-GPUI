//! The session layer (S7.1.1, D13, D24): the values the player's settings panel changed, which
//! win over their defaults until they are reset, saved as defaults, or VLC quits.

use std::collections::BTreeMap;

use crate::settings::Settings;
use crate::settings::registry::{self, Value};

/// The panel's values, each by its setting's registry key. A key is here once the panel changed
/// it, even back to its default: from then on a default changed in Settings doesn't move it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Session {
    values: BTreeMap<&'static str, Value>,
}

impl Session {
    /// Holds `value` for the setting `key` for the rest of the session.
    pub fn set(&mut self, key: &'static str, value: Value) {
        self.values.insert(key, value);
    }

    /// Lets each of `keys` follow its default again; answers whether any was held.
    pub fn reset<'a>(&mut self, keys: impl IntoIterator<Item = &'a str>) -> bool {
        let mut any = false;
        for key in keys {
            any |= self.values.remove(key).is_some();
        }
        any
    }

    /// Whether the panel holds a value for `key`.
    pub fn is_changed(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    /// Takes the held values of `keys` out of the session, to save them as defaults.
    pub fn take<'a>(
        &mut self,
        keys: impl IntoIterator<Item = &'a str>,
    ) -> Vec<(&'static str, Value)> {
        keys.into_iter().filter_map(|key| self.values.remove_entry(key)).collect()
    }

    /// `defaults` with the held values written over them: the settings in effect.
    pub fn overlay(&self, defaults: &Settings) -> Settings {
        let mut effective = defaults.clone();
        write(&mut effective, self.values.iter().map(|(key, value)| (*key, value.clone())));
        effective
    }
}

/// Writes each value into `settings` through its setting's binding; unknown keys are skipped.
///
/// Switches go last: a choice may imply one (an equalizer preset turns the equalizer on,
/// S7.7.3), and a switch the panel holds off must stay off. Otherwise values go in order, so a
/// section lists a control another one overrides first (the equalizer's gains, then its preset).
pub fn write(settings: &mut Settings, values: impl IntoIterator<Item = (&'static str, Value)>) {
    let (switches, others): (Vec<_>, Vec<_>) =
        values.into_iter().partition(|(_, value)| matches!(value, Value::Bool(_)));
    for (key, value) in others.into_iter().chain(switches) {
        if let Some(def) = registry::find(key) {
            (def.binding.set)(settings, value);
        } else {
            tracing::warn!("effects: no setting {key}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BRIGHTNESS: &str = "video.picture.brightness";
    const ENABLED: &str = "video.picture.enabled";

    #[test]
    fn a_held_value_wins_over_its_default_until_reset() {
        let mut session = Session::default();
        let mut defaults = Settings::default();
        session.set(BRIGHTNESS, Value::Number(1.5));
        defaults.prefs.video.picture.brightness = 0.5;
        let effective = session.overlay(&defaults);
        assert!((effective.prefs.video.picture.brightness - 1.5).abs() < f32::EPSILON);
        assert!(session.reset([BRIGHTNESS, ENABLED]));
        assert!(!session.reset([BRIGHTNESS]), "nothing left to reset");
        let effective = session.overlay(&defaults);
        assert!((effective.prefs.video.picture.brightness - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn taking_values_leaves_the_others_held() {
        let mut session = Session::default();
        session.set(BRIGHTNESS, Value::Number(1.5));
        session.set(ENABLED, Value::Bool(true));
        let taken = session.take([BRIGHTNESS, "video.picture.gamma"]);
        assert_eq!(taken, [(BRIGHTNESS, Value::Number(1.5))]);
        assert!(session.is_changed(ENABLED));
        assert!(!session.is_changed(BRIGHTNESS));
    }

    #[test]
    fn a_switch_held_off_wins_over_a_choice_that_turns_it_on() {
        let mut session = Session::default();
        session.set("audio.equalizer", Value::Bool(false));
        session.set("audio.equalizer_preset", Value::Text("rock".into()));
        let effective = session.overlay(&Settings::default());
        assert!(!effective.prefs.audio.equalizer);
        assert_eq!(
            effective.prefs.audio.equalizer_preset,
            crate::engine::audio::equalizer::Preset::Rock
        );
    }

    #[test]
    fn writing_goes_through_the_bindings() {
        let mut settings = Settings::default();
        write(&mut settings, [(ENABLED, Value::Bool(true)), ("no.such.key", Value::Bool(true))]);
        assert!(settings.prefs.video.picture.enabled);
    }
}
