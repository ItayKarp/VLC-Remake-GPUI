//! Audio's Loudness and Effects sections (S5.8.2, `audio_01.png`, `audio_02.png`): the loudness
//! rows (S7.9.4), the equalizer (S7.7) and spatial audio; the Compressor, Spatializer and Stereo
//! widening follow in `audio_sections.rs`. Every row is heard at once. The preset reads None while
//! the equalizer is off ([`EqualizerPick`]); the bands stay usable then, as designed, and moving
//! one turns the equalizer on, as moving a panel control turns its section on (D23).

use gpui::App;

use super::super::def::{Condition, Control, MachineChoice, SettingDef};
use super::super::value::{Binding, Choice, SettingChoice, Value, choice, number, toggle};
use crate::assets::equalizer_glyph;
use crate::engine::audio::effects::{ReplayGain, SpatialMode};
use crate::engine::audio::equalizer::Equalizer;
use crate::settings::registry::Category;
use crate::settings::{self, EqualizerPick};

const LOUDNESS: (Category, &str) = (Category::Audio, tr_noop!("Loudness"));
const EFFECTS: (Category, &str) = (Category::Audio, tr_noop!("Effects"));
/// A gain field.
const DECIBELS: Control = Control::Number { unit: tr_noop!("dB") };

impl SettingChoice for ReplayGain {
    const VALUES: &'static [Self] = &[Self::Off, Self::Track, Self::Album];
    const CHOICES: &'static [Choice] = &[
        Choice::new("off", tr_noop!("Off")),
        Choice::new("track", tr_noop!("Track")),
        Choice::new("album", tr_noop!("Album")),
    ];
}

impl SettingChoice for SpatialMode {
    const VALUES: &'static [Self] = &[Self::HeadphoneVirtualization, Self::Binaural];
    const CHOICES: &'static [Choice] = &[
        Choice::new("headphone_virtualization", tr_noop!("Headphone virtualization")),
        Choice::new("binaural", tr_noop!("Binaural (360° audio)")),
    ];
}

/// "Equalizer preset": None (the equalizer off), Custom, VLC's presets and the saved ones, by
/// [`EqualizerPick`] id. Picking one turns the equalizer on with its gains; None turns it off.
const EQUALIZER_PRESET: Binding = Binding {
    get: |s| Value::Text(s.prefs.equalizer_pick().id()),
    set: |s, value| {
        let Value::Text(id) = value else { return };
        if let Some(pick) = EqualizerPick::from_id(&id) {
            s.prefs.pick_equalizer(&pick);
        }
    },
};

/// The equalizer's gains: the preamp, then the bands. Setting them leaves the preset to the
/// caller, which says Custom when a band moved.
const EQUALIZER_GAINS: Binding = Binding {
    get: |s| Value::Numbers(s.prefs.audio.equalizer_gains().gains().map(f64::from).collect()),
    set: |s, value| {
        let Value::Numbers(gains) = value else { return };
        let gains: Vec<f32> = gains.into_iter().map(|gain| gain as f32).collect();
        if let Some(gains) = Equalizer::from_gains(&gains) {
            let audio = &mut s.prefs.audio;
            audio.equalizer_preamp = gains.preamp;
            audio.equalizer_bands = gains.bands;
        }
    },
};

/// The presets dropdown's entries, with each preset's curve glyph.
fn equalizer_presets(cx: &App, _saved: &str) -> Vec<MachineChoice> {
    settings::get(cx).prefs.equalizer_picks().iter().map(preset_choice).collect()
}

/// `pick` as a dropdown entry.
fn preset_choice(pick: &EqualizerPick) -> MachineChoice {
    let glyph = match pick {
        EqualizerPick::Preset(preset) => equalizer_glyph(*preset),
        EqualizerPick::None | EqualizerPick::Saved(_) => None,
    };
    MachineChoice { id: pick.id(), label: pick.label(), glyph }
}

/// The Audio page's Loudness and Effects rows.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    SettingDef::new(
        "audio.normalize_loudness",
        LOUDNESS,
        tr_noop!("Normalize loudness"),
        Control::Toggle,
        toggle!(prefs.audio.normalize_loudness),
    )
    .info(tr_noop!(
        "Turns loud passages down to the target level, so loud and quiet media sound alike. Heard \
         at once."
    ))
    .keywords(&["volume", "level", "normalizer", "loud"]),
    SettingDef::new(
        "audio.loudness_target",
        LOUDNESS,
        tr_noop!("Target level"),
        Control::Number { unit: "" },
        number!(f32: prefs.audio.loudness_target),
    )
    .info(tr_noop!(
        "How loud sound may get before it is turned down: lower is quieter. VLC uses 2."
    ))
    .range(0.5, 10., 0.1)
    .depends_on("audio.normalize_loudness", Condition::IsOn),
    SettingDef::new(
        "audio.night_mode",
        LOUDNESS,
        tr_noop!("Night mode"),
        Control::Toggle,
        toggle!(prefs.audio.night_mode),
    )
    .info(tr_noop!(
        "Softens explosions and music and lifts quiet dialogue, for listening at low volume: a \
         compressor preset, unless the Compressor below is on."
    ))
    .keywords(&["compressor", "dynamic range", "dialogue", "quiet"]),
    SettingDef::new(
        "audio.replay_gain",
        LOUDNESS,
        tr_noop!("Replay gain"),
        Control::Dropdown(ReplayGain::CHOICES),
        choice!(prefs.audio.replay_gain),
    )
    .info(tr_noop!(
        "Uses the loudness tags in music files: Track makes every track equally loud, Album keeps \
         an album's own loud and quiet tracks. Files without tags play as they are."
    ))
    .keywords(&["replaygain", "music", "tags", "volume"]),
    SettingDef::new(
        "audio.replay_gain_preamp",
        LOUDNESS,
        tr_noop!("Replay gain preamp"),
        DECIBELS,
        number!(f32: prefs.audio.replay_gain_preamp),
    )
    .info(tr_noop!(
        "Added to the tagged gain, in decibels; raise it if tagged music plays too quietly."
    ))
    .range(-20., 20., 0.1)
    .depends_on("audio.replay_gain", Condition::IsNot("off")),
    SettingDef::new(
        "audio.equalizer",
        EFFECTS,
        tr_noop!("Equalizer"),
        Control::Toggle,
        toggle!(prefs.audio.equalizer),
    )
    .info(tr_noop!(
        "Shapes the sound with the bands below, from bass on the left to treble on the right."
    ))
    .keywords(&["eq", "bass", "treble", "bands"]),
    SettingDef::new(
        "audio.equalizer_preset",
        EFFECTS,
        tr_noop!("Equalizer preset"),
        Control::Options(equalizer_presets),
        EQUALIZER_PRESET,
    )
    .info(tr_noop!(
        "Sets the bands for a kind of music; None turns the equalizer off. Moving a band \
         afterwards says Custom."
    ))
    .keywords(&["eq", "rock", "pop", "classical", "custom"]),
    SettingDef::new(
        "audio.equalizer_bands",
        EFFECTS,
        tr_noop!("Equalizer bands"),
        Control::Equalizer,
        EQUALIZER_GAINS,
    )
    .info(tr_noop!("The preamp and ten bands, each from -20 to 20 decibels."))
    .keywords(&["eq", "preamp", "bass", "treble", "hz"]),
    SettingDef::new(
        "audio.equalizer_two_pass",
        EFFECTS,
        tr_noop!("2 Pass"),
        Control::Toggle,
        toggle!(prefs.audio.equalizer_two_pass),
    )
    .info(tr_noop!("Runs the equalizer twice, for a stronger effect."))
    .depends_on("audio.equalizer", Condition::IsOn)
    .keywords(&["eq", "stronger", "two-pass", "twice"]),
    SettingDef::new(
        "audio.spatial_audio",
        EFFECTS,
        tr_noop!("Spatial audio"),
        Control::Toggle,
        toggle!(prefs.audio.spatial_audio),
    )
    .info(tr_noop!("Places sound around you in headphones, as the mode below says."))
    .keywords(&["3d", "surround", "headphones", "virtual"]),
    SettingDef::new(
        "audio.spatial_mode",
        EFFECTS,
        tr_noop!("Spatial audio mode"),
        Control::Dropdown(SpatialMode::CHOICES),
        choice!(prefs.audio.spatial_mode),
    )
    .info(tr_noop!(
        "Headphone virtualization plays surround (5.1, 7.1) as if from speakers around you. \
         Binaural renders 360° (ambisonic) sound for headphones."
    ))
    .depends_on("audio.spatial_audio", Condition::IsOn)
    .keywords(&["ambisonic", "360", "binaural", "hrtf"]),
];
