//! Audio's Loudness and Effects sections (S5.8.2, `audio_01.png`, `audio_02.png`).
//!
//! libVLC 3 sets up its audio filters, replay gain and the equalizer's two passes when its output
//! starts, from options the player's instance is made with, so those rows apply at the next
//! launch. The equalizer, its preset and its bands change what plays at once. The preset and
//! bands stay usable while the equalizer is off, as designed, so they can be set up first.

use super::super::def::{ApplyMode, Condition, Control, SettingDef};
use super::super::value::{Binding, Choice, SettingChoice, Value, choice, number, toggle};
use crate::engine::audio::effects::{ReplayGain, SpatialMode};
use crate::engine::audio::equalizer::Preset;
use crate::settings::registry::Category;

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

impl SettingChoice for Preset {
    const VALUES: &'static [Self] = &[
        Self::None,
        Self::Flat,
        Self::Classical,
        Self::Club,
        Self::Dance,
        Self::FullBass,
        Self::FullBassAndTreble,
        Self::FullTreble,
        Self::Headphones,
        Self::LargeHall,
        Self::Live,
        Self::Party,
        Self::Pop,
        Self::Reggae,
        Self::Rock,
        Self::Ska,
        Self::Soft,
        Self::SoftRock,
        Self::Techno,
    ];
    const CHOICES: &'static [Choice] = &[
        Choice::new("none", "None"),
        Choice::new("flat", "Flat"),
        Choice::new("classical", "Classical"),
        Choice::new("club", "Club"),
        Choice::new("dance", "Dance"),
        Choice::new("full_bass", "Full bass"),
        Choice::new("full_bass_and_treble", "Full bass and treble"),
        Choice::new("full_treble", "Full treble"),
        Choice::new("headphones", "Headphones"),
        Choice::new("large_hall", "Large hall"),
        Choice::new("live", "Live"),
        Choice::new("party", "Party"),
        Choice::new("pop", "Pop"),
        Choice::new("reggae", "Reggae"),
        Choice::new("rock", "Rock"),
        Choice::new("ska", "Ska"),
        Choice::new("soft", "Soft"),
        Choice::new("soft_rock", "Soft rock"),
        Choice::new("techno", "Techno"),
    ];
}

/// "Equalizer preset": picking one sets the bands to it; None leaves them as they are.
const EQUALIZER_PRESET: Binding = Binding {
    get: |s| Value::Choice(s.prefs.audio.equalizer_preset.choice_id()),
    set: |s, value| {
        let Value::Choice(id) = value else { return };
        let Some(preset) = Preset::from_choice_id(id) else { return };
        match preset.equalizer() {
            Some(gains) => s.prefs.audio.set_equalizer_gains(gains, preset),
            None => s.prefs.audio.equalizer_preset = Preset::None,
        }
    },
};

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
        Control::Dropdown(Preset::CHOICES),
        EQUALIZER_PRESET,
    )
    .info("Sets the bands for a kind of music. Moving a band afterwards sets this back to None.")
    .keywords(&["eq", "rock", "pop", "classical"]),
    SettingDef::new(
        "audio.equalizer_bands",
        EFFECTS,
        "Equalizer bands",
        Control::Equalizer,
        Binding::ACTION,
    )
    .info("The preamp and ten bands, each from -20 to 20 decibels.")
    .keywords(&["eq", "preamp", "bass", "treble", "hz"]),
    SettingDef::new(
        "audio.equalizer_two_pass",
        EFFECTS,
        "Two-pass",
        Control::Toggle,
        toggle!(prefs.audio.equalizer_two_pass),
    )
    .info("Runs the equalizer twice, for a stronger effect.")
    .depends_on("audio.equalizer", Condition::IsOn)
    .applies(ApplyMode::Restart)
    .keywords(&["eq", "stronger"]),
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
