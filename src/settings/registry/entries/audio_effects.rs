//! Audio's Loudness and Effects sections (S5.8.2, `audio_01.png`, `audio_02.png`).
//!
//! libVLC 3 sets up its audio filters and replay gain when its output starts, from options the
//! player's instance is made with, so those rows apply at the next launch. The equalizer, its
//! preset, its bands and its 2 Pass change what plays at once (S7.7). The preset reads None while
//! the equalizer is off ([`EqualizerPick`]); the bands stay usable then, as designed, and moving
//! one turns the equalizer on, as moving a panel control turns its section on (D23).

use gpui::App;

use super::super::def::{ApplyMode, Condition, Control, MachineChoice, SettingDef};
use super::super::value::{Binding, Choice, SettingChoice, Value, choice, number, toggle};
use crate::assets::equalizer_glyph;
use crate::engine::audio::effects::{ReplayGain, SpatialMode};
use crate::engine::audio::equalizer::Equalizer;
use crate::settings::registry::Category;
use crate::settings::{self, EqualizerPick};

const LOUDNESS: (Category, &str) = (Category::Audio, "Loudness");
const EFFECTS: (Category, &str) = (Category::Audio, "Effects");
/// A gain field.
const DECIBELS: Control = Control::Number { unit: "dB" };

impl SettingChoice for ReplayGain {
    const VALUES: &'static [Self] = &[Self::Off, Self::Track, Self::Album];
    const CHOICES: &'static [Choice] =
        &[Choice::new("off", "Off"), Choice::new("track", "Track"), Choice::new("album", "Album")];
}

impl SettingChoice for SpatialMode {
    const VALUES: &'static [Self] = &[Self::HeadphoneVirtualization, Self::Binaural];
    const CHOICES: &'static [Choice] = &[
        Choice::new("headphone_virtualization", "Headphone virtualization"),
        Choice::new("binaural", "Binaural (360° audio)"),
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
        "Normalize loudness",
        Control::Toggle,
        toggle!(prefs.audio.normalize_loudness),
    )
    .info("Turns loud passages down to the target level, so loud and quiet media sound alike.")
    .applies(ApplyMode::Restart)
    .keywords(&["volume", "level", "normalizer", "loud"]),
    SettingDef::new(
        "audio.loudness_target",
        LOUDNESS,
        "Target level",
        Control::Number { unit: "" },
        number!(f32: prefs.audio.loudness_target),
    )
    .info("How loud sound may get before it is turned down: lower is quieter. VLC uses 2.")
    .range(0.5, 10., 0.1)
    .depends_on("audio.normalize_loudness", Condition::IsOn)
    .applies(ApplyMode::Restart),
    SettingDef::new(
        "audio.night_mode",
        LOUDNESS,
        "Night mode",
        Control::Toggle,
        toggle!(prefs.audio.night_mode),
    )
    .info("Softens explosions and music and lifts quiet dialogue, for listening at low volume.")
    .applies(ApplyMode::Restart)
    .keywords(&["compressor", "dynamic range", "dialogue", "quiet"]),
    SettingDef::new(
        "audio.replay_gain",
        LOUDNESS,
        "Replay gain",
        Control::Dropdown(ReplayGain::CHOICES),
        choice!(prefs.audio.replay_gain),
    )
    .info(
        "Uses the loudness tags in music files: Track makes every track equally loud, Album \
         keeps an album's own loud and quiet tracks.",
    )
    .applies(ApplyMode::Restart)
    .keywords(&["replaygain", "music", "tags", "volume"]),
    SettingDef::new(
        "audio.replay_gain_preamp",
        LOUDNESS,
        "Replay gain preamp",
        DECIBELS,
        number!(f32: prefs.audio.replay_gain_preamp),
    )
    .info("Added to the tagged gain, in decibels; raise it if tagged music plays too quietly.")
    .range(-20., 20., 0.1)
    .depends_on("audio.replay_gain", Condition::IsNot("off"))
    .applies(ApplyMode::Restart),
    SettingDef::new(
        "audio.equalizer",
        EFFECTS,
        "Equalizer",
        Control::Toggle,
        toggle!(prefs.audio.equalizer),
    )
    .info("Shapes the sound with the bands below, from bass on the left to treble on the right.")
    .keywords(&["eq", "bass", "treble", "bands"]),
    SettingDef::new(
        "audio.equalizer_preset",
        EFFECTS,
        "Equalizer preset",
        Control::Options(equalizer_presets),
        EQUALIZER_PRESET,
    )
    .info(
        "Sets the bands for a kind of music; None turns the equalizer off. Moving a band \
         afterwards says Custom.",
    )
    .keywords(&["eq", "rock", "pop", "classical", "custom"]),
    SettingDef::new(
        "audio.equalizer_bands",
        EFFECTS,
        "Equalizer bands",
        Control::Equalizer,
        EQUALIZER_GAINS,
    )
    .info("The preamp and ten bands, each from -20 to 20 decibels.")
    .keywords(&["eq", "preamp", "bass", "treble", "hz"]),
    SettingDef::new(
        "audio.equalizer_two_pass",
        EFFECTS,
        "2 Pass",
        Control::Toggle,
        toggle!(prefs.audio.equalizer_two_pass),
    )
    .info("Runs the equalizer twice, for a stronger effect.")
    .depends_on("audio.equalizer", Condition::IsOn)
    .keywords(&["eq", "stronger", "two-pass", "twice"]),
    SettingDef::new(
        "audio.spatial_audio",
        EFFECTS,
        "Spatial audio",
        Control::Toggle,
        toggle!(prefs.audio.spatial_audio),
    )
    .info("Places sound around you in headphones, as the mode below says.")
    .applies(ApplyMode::Restart)
    .keywords(&["3d", "surround", "headphones", "virtual"]),
    SettingDef::new(
        "audio.spatial_mode",
        EFFECTS,
        "Spatial audio mode",
        Control::Dropdown(SpatialMode::CHOICES),
        choice!(prefs.audio.spatial_mode),
    )
    .info(
        "Headphone virtualization plays surround (5.1, 7.1) as if from speakers around you. \
         Binaural renders 360° (ambisonic) sound for headphones.",
    )
    .depends_on("audio.spatial_audio", Condition::IsOn)
    .applies(ApplyMode::Restart)
    .keywords(&["ambisonic", "360", "binaural", "hrtf"]),
    SettingDef::new(
        "audio.stereo_widening",
        EFFECTS,
        "Stereo widening",
        Control::Toggle,
        toggle!(prefs.audio.stereo_widening),
    )
    .info("Makes stereo sound wider than the speakers or headphones are apart.")
    .applies(ApplyMode::Restart)
    .keywords(&["stereo", "wide", "spread"]),
];
