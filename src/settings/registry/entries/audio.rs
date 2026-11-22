//! Audio ("Audio Defaults"): the output, the volume and the preferred languages.

use super::super::def::{ApplyMode, Condition, Control, SettingDef};
use super::super::value::{Binding, Choice, SettingChoice, Value, choice, number, toggle};
use super::languages;
use crate::engine::audio::SpeakerSetup;
use crate::settings::StartupVolume;
use crate::settings::registry::Category;

const OUTPUT: (Category, &str) = (Category::Audio, "Output");
const VOLUME: (Category, &str) = (Category::Audio, "Volume");
const TRACKS: (Category, &str) = (Category::Audio, "Tracks & Languages");
/// A percentage field.
const PERCENT: Control = Control::Number { unit: "%" };

impl SettingChoice for SpeakerSetup {
    const VALUES: &'static [Self] = &[Self::Automatic, Self::Stereo];
    const CHOICES: &'static [Choice] =
        &[Choice::new("automatic", "Automatic"), Choice::new("stereo", "Stereo")];
}

impl SettingChoice for StartupVolume {
    const VALUES: &'static [Self] = &[Self::RememberLast, Self::Fixed];
    const CHOICES: &'static [Choice] = &[
        Choice::new("remember_last", "Remember last volume"),
        Choice::new("fixed", "Always the same"),
    ];
}

/// A volume as the slider labels it: "200%".
fn percent(value: f64) -> String {
    format!("{value:.0}%")
}

/// "Audio language": the preferred languages, most preferred first.
const AUDIO_LANGUAGES: Binding = Binding {
    get: |s| Value::List(s.prefs.audio.preferred_languages.clone()),
    set: |s, value| {
        if let Value::List(languages) = value {
            s.prefs.audio.preferred_languages = languages;
        }
    },
};

/// The Audio page's rows.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    SettingDef::new(
        "audio.speaker_setup",
        OUTPUT,
        "Speaker setup",
        Control::Dropdown(SpeakerSetup::CHOICES),
        choice!(prefs.audio.speaker_setup),
    )
    .info(
        "Automatic plays surround sound as surround on a surround device and mixes it down for \
         stereo. Stereo always mixes it down to two channels.",
    )
    .keywords(&["channels", "surround", "5.1", "7.1", "downmix"]),
    SettingDef::new(
        "audio.pause_on_device_disconnect",
        OUTPUT,
        "Pause when headphones are unplugged",
        Control::Toggle,
        toggle!(prefs.audio.pause_on_device_disconnect),
    )
    .info("Pauses when the device that was playing goes away, so sound doesn't jump to speakers.")
    .keywords(&["bluetooth", "disconnect", "output device"]),
    SettingDef::new(
        "audio.passthrough",
        OUTPUT,
        "Passthrough",
        Control::Toggle,
        toggle!(prefs.audio.passthrough),
    )
    .info(
        "Sends Dolby Digital (AC3) and DTS sound undecoded to a receiver over HDMI or S/PDIF, \
         for the receiver to decode.",
    )
    .applies(ApplyMode::Restart)
    .keywords(&["spdif", "hdmi", "ac3", "dts", "receiver", "bitstream"])
    .advanced(),
    SettingDef::new(
        "audio.max_volume_percent",
        VOLUME,
        "Maximum volume",
        Control::Slider { marks: Some(50.), format: percent },
        number!(u32: prefs.audio.max_volume_percent),
    )
    .info("How far the volume goes. Above 100%, quiet media gets louder but may distort.")
    .range(100., 300., 5.)
    .keywords(&["boost", "amplify", "louder"]),
    SettingDef::new(
        "audio.startup_volume",
        VOLUME,
        "Startup volume",
        Control::Dropdown(StartupVolume::CHOICES),
        choice!(prefs.audio.startup_volume),
    )
    .info(
        "The volume VLC starts at: the last one (at most 100%, so a boost doesn't carry over), \
         or always the same.",
    ),
    SettingDef::new(
        "audio.startup_volume_percent",
        VOLUME,
        "Always starts at",
        PERCENT,
        number!(u32: prefs.audio.startup_volume_percent),
    )
    .info("The volume each launch starts at, up to the maximum volume.")
    .range(0., 300., 1.)
    .depends_on("audio.startup_volume", Condition::Is("fixed")),
    SettingDef::new(
        "audio.volume_step_percent",
        VOLUME,
        "Volume step",
        PERCENT,
        number!(u32: prefs.audio.volume_step_percent),
    )
    .info("How much the Up and Down keys and each scroll step change the volume.")
    .range(1., 100., 1.)
    .keywords(&["increment", "arrow keys", "scroll"]),
    SettingDef::new(
        "audio.preferred_languages",
        TRACKS,
        "Audio language",
        Control::List,
        AUDIO_LANGUAGES,
    )
    .info(
        "Language codes such as en or fra, most preferred first, separated by commas. New media \
         plays the first one it has; empty keeps the media's own choice.",
    )
    .validate(languages)
    .applies(ApplyMode::NextMedia)
    .keywords(&["dub", "track", "language"]),
];
