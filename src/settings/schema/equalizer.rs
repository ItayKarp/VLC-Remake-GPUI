//! What the equalizer's presets dropdown picks (S7.7.3, S7.7.4), and the presets the user saved.
//!
//! **None and Flat.** "None" is the equalizer off: sound plays untouched, the bands kept for
//! when it comes back on. "Flat" is the equalizer on with VLC's Flat gains (a 12 dB preamp, every
//! band at 0 dB), which passes the sound through the filter. So the dropdown shows None whenever
//! the equalizer is off, picking None turns it off, and picking anything else turns it on.
//!
//! **Custom.** Moving a band leaves whatever preset the gains came from: the dropdown says
//! "Custom". "Save as preset…" names the gains in use; a saved preset is picked like VLC's, and
//! moving a band leaves it for Custom too.

use serde::{Deserialize, Serialize};

use super::super::lenient::lenient;
use super::Preferences;
use super::audio::AudioSettings;
use crate::engine::audio::equalizer::{BANDS, Equalizer, Preset};

/// The prefix of a saved preset's id in the dropdown.
const SAVED: &str = "saved:";
/// The longest name a saved preset takes, in characters.
pub const MAX_SAVED_NAME: usize = 40;

/// A preset the user saved: a name and its gains.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SavedEqualizer {
    /// The name the dropdown lists.
    #[serde(deserialize_with = "lenient")]
    pub name: String,
    /// The preamp, in dB.
    #[serde(deserialize_with = "lenient")]
    pub preamp: f32,
    /// The bands, in dB, lowest frequency first.
    #[serde(deserialize_with = "lenient")]
    pub bands: [f32; BANDS],
}

impl SavedEqualizer {
    /// Its gains, kept within range whatever the file says (an edited file, S7.7 edge cases).
    pub fn gains(&self) -> Equalizer {
        Equalizer { preamp: self.preamp, bands: self.bands }.clamped()
    }
}

impl Default for SavedEqualizer {
    fn default() -> Self {
        Self { name: String::new(), preamp: Equalizer::FLAT.preamp, bands: Equalizer::FLAT.bands }
    }
}

/// One entry of the presets dropdown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EqualizerPick {
    /// The equalizer off.
    None,
    /// One of VLC's presets, or Custom.
    Preset(Preset),
    /// A preset the user saved, by name.
    Saved(String),
}

impl EqualizerPick {
    /// The entry's id: `none`, a preset's id, or `saved:` and the name.
    pub fn id(&self) -> String {
        match self {
            Self::None => "none".to_owned(),
            Self::Preset(preset) => preset.id().to_owned(),
            Self::Saved(name) => format!("{SAVED}{name}"),
        }
    }

    /// The entry with id `id`.
    pub fn from_id(id: &str) -> Option<Self> {
        if id == "none" {
            return Some(Self::None);
        }
        match id.strip_prefix(SAVED) {
            Some(name) => Some(Self::Saved(name.to_owned())),
            None => Preset::from_id(id).map(Self::Preset),
        }
    }

    /// What the dropdown shows.
    pub fn label(&self) -> String {
        match self {
            Self::None => "None".to_owned(),
            Self::Preset(preset) => preset.label().to_owned(),
            Self::Saved(name) => name.clone(),
        }
    }
}

impl AudioSettings {
    /// The equalizer's gains as set.
    pub fn equalizer_gains(&self) -> Equalizer {
        Equalizer { preamp: self.equalizer_preamp, bands: self.equalizer_bands }
    }

    /// Sets the equalizer's gains; `preset` is the preset they came from (Custom for the
    /// user's own), and they come from no saved preset.
    pub fn set_equalizer_gains(&mut self, gains: Equalizer, preset: Preset) {
        self.equalizer_preamp = gains.preamp;
        self.equalizer_bands = gains.bands;
        self.equalizer_preset = preset;
        self.equalizer_saved_name.clear();
    }
}

impl Preferences {
    /// The dropdown's entry for what plays: None while the equalizer is off.
    pub fn equalizer_pick(&self) -> EqualizerPick {
        let audio = &self.audio;
        if !audio.equalizer {
            return EqualizerPick::None;
        }
        let saved = &audio.equalizer_saved_name;
        if audio.equalizer_preset == Preset::Custom && self.saved_equalizer(saved).is_some() {
            return EqualizerPick::Saved(saved.clone());
        }
        EqualizerPick::Preset(audio.equalizer_preset)
    }

    /// Picks `pick`: None turns the equalizer off, leaving the gains; anything else turns it on
    /// with that entry's gains (Custom keeps them). A saved preset that is gone changes nothing.
    pub fn pick_equalizer(&mut self, pick: &EqualizerPick) {
        match pick {
            EqualizerPick::None => self.audio.equalizer = false,
            EqualizerPick::Preset(preset) => {
                let gains = preset.equalizer().unwrap_or_else(|| self.audio.equalizer_gains());
                self.audio.set_equalizer_gains(gains, *preset);
                self.audio.equalizer = true;
            }
            EqualizerPick::Saved(name) => {
                let Some(gains) = self.saved_equalizer(name).map(SavedEqualizer::gains) else {
                    return;
                };
                self.audio.set_equalizer_gains(gains, Preset::Custom);
                self.audio.equalizer_saved_name.clone_from(name);
                self.audio.equalizer = true;
            }
        }
    }

    /// The saved preset named `name`.
    pub fn saved_equalizer(&self, name: &str) -> Option<&SavedEqualizer> {
        self.equalizer_presets.iter().find(|saved| saved.name == name)
    }

    /// Saves `gains` as the preset `name` (trimmed), replacing one of that name; `None` for a
    /// name that is empty or too long, else the name saved.
    pub fn save_equalizer(&mut self, name: &str, gains: Equalizer) -> Option<String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > MAX_SAVED_NAME {
            return None;
        }
        let saved =
            SavedEqualizer { name: name.to_owned(), preamp: gains.preamp, bands: gains.bands };
        match self.equalizer_presets.iter_mut().find(|other| other.name == name) {
            Some(other) => *other = saved,
            None => self.equalizer_presets.push(saved),
        }
        Some(name.to_owned())
    }

    /// Every entry of the dropdown: None, Custom, VLC's presets, then the saved ones.
    pub fn equalizer_picks(&self) -> Vec<EqualizerPick> {
        let presets = std::iter::once(Preset::Custom).chain(Preset::ALL).map(EqualizerPick::Preset);
        let saved =
            self.equalizer_presets.iter().map(|saved| EqualizerPick::Saved(saved.name.clone()));
        std::iter::once(EqualizerPick::None).chain(presets).chain(saved).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_is_the_equalizer_off_and_keeps_the_gains() {
        let mut prefs = Preferences::default();
        assert_eq!(prefs.equalizer_pick(), EqualizerPick::None);
        prefs.pick_equalizer(&EqualizerPick::Preset(Preset::Rock));
        assert!(prefs.audio.equalizer);
        assert_eq!(prefs.equalizer_pick(), EqualizerPick::Preset(Preset::Rock));
        prefs.pick_equalizer(&EqualizerPick::None);
        assert!(!prefs.audio.equalizer);
        assert_eq!(prefs.audio.equalizer_gains(), Preset::Rock.equalizer().expect("a preset"));
    }

    #[test]
    fn flat_plays_through_the_equalizer() {
        let mut prefs = Preferences::default();
        prefs.pick_equalizer(&EqualizerPick::Preset(Preset::Flat));
        assert!(prefs.audio.equalizer);
        assert_eq!(prefs.audio.equalizer_gains(), Equalizer::FLAT);
    }

    #[test]
    fn a_saved_preset_is_picked_by_name_and_clamped() {
        let mut prefs = Preferences::default();
        let loud = Equalizer { preamp: 30., bands: [3.; BANDS] };
        assert_eq!(prefs.save_equalizer("  Loud ", loud), Some("Loud".to_owned()));
        assert_eq!(prefs.save_equalizer(" ", loud), None);
        prefs.pick_equalizer(&EqualizerPick::Saved("Loud".into()));
        assert_eq!(prefs.equalizer_pick(), EqualizerPick::Saved("Loud".into()));
        assert_eq!(prefs.audio.equalizer_preamp, 20.);
        prefs
            .audio
            .set_equalizer_gains(prefs.audio.equalizer_gains().with_gain(1, 0.), Preset::Custom);
        assert_eq!(prefs.equalizer_pick(), EqualizerPick::Preset(Preset::Custom));
    }

    #[test]
    fn picks_read_back_from_their_ids() {
        let mut prefs = Preferences::default();
        prefs.save_equalizer("Mine", Equalizer::FLAT);
        let picks = prefs.equalizer_picks();
        assert_eq!(picks.len(), 1 + 1 + Preset::ALL.len() + 1);
        for pick in picks {
            assert_eq!(EqualizerPick::from_id(&pick.id()), Some(pick));
        }
    }
}
