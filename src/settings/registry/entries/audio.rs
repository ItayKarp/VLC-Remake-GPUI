//! Audio ("Audio Defaults", S5.8.2): the output and the volume, and the preferred languages,
//! which the page shows last. Loudness and effects are in `audio_effects.rs`.

use super::super::def::{ApplyMode, Condition, Control, Marks, SettingDef};
use super::super::value::{Binding, Choice, SettingChoice, Value, choice, number, text, toggle};
use super::languages;
use crate::app::audio_devices;
use crate::engine::audio::visualizer::Visualizer;
use crate::engine::audio::{SpeakerSetup, StereoMode};
use crate::settings::registry::Category;
use crate::settings::{self, Settings, StartupVolume};

const OUTPUT: (Category, &str) = (Category::Audio, tr_noop!("Output"));
const VOLUME: (Category, &str) = (Category::Audio, tr_noop!("Volume"));
const TRACKS: (Category, &str) = (Category::Audio, tr_noop!("Tracks & Languages"));
const VISUALIZATION: (Category, &str) = (Category::Audio, tr_noop!("Visualization"));
/// A percentage field.
const PERCENT: Control = Control::Number { unit: tr_noop!("%") };
/// Maximum volume's ticks, and the ticks that are labelled.
const VOLUME_TICK: f64 = 25.;
const VOLUME_LABEL: f64 = 100.;

impl SettingChoice for SpeakerSetup {
    const VALUES: &'static [Self] = &[Self::Automatic, Self::Stereo];
    const CHOICES: &'static [Choice] = &[
        Choice::new("automatic", tr_noop!("Automatic")),
        Choice::new("stereo", tr_noop!("Stereo")),
    ];
}

impl SettingChoice for StereoMode {
    const VALUES: &'static [Self] = &[
        Self::Stereo,
        Self::Mono,
        Self::Left,
        Self::Right,
        Self::ReverseStereo,
        Self::DolbySurround,
        Self::Headphones,
    ];
    const CHOICES: &'static [Choice] = &[
        Choice::new("stereo", tr_noop!("Stereo")),
        Choice::new("mono", tr_noop!("Mono")),
        Choice::new("left", tr_noop!("Left")),
        Choice::new("right", tr_noop!("Right")),
        Choice::new("reverse_stereo", tr_noop!("Reverse stereo")),
        Choice::new("dolby_surround", tr_noop!("Dolby Surround")),
        Choice::new("headphones", tr_noop!("Headphones")),
    ];
}

impl SettingChoice for Visualizer {
    const VALUES: &'static [Self] = &Self::ALL;
    const CHOICES: &'static [Choice] = &[
        Choice::new("disabled", tr_noop!("Disabled")),
        Choice::new("spectrometer", tr_noop!("Spectrometer")),
        Choice::new("scope", tr_noop!("Scope")),
        Choice::new("spectrum", tr_noop!("Spectrum")),
        Choice::new("waveform", tr_noop!("Waveform")),
    ];
}

impl SettingChoice for StartupVolume {
    const VALUES: &'static [Self] = &[Self::RememberLast, Self::Fixed];
    const CHOICES: &'static [Choice] = &[
        Choice::new("remember_last", tr_noop!("Remember last volume")),
        Choice::new("fixed", tr_noop!("Always the same")),
    ];
}

/// A volume as the slider labels it: "200%".
fn percent(value: f64) -> String {
    tr!("{value}%", value = crate::i18n::decimal(value, 0))
}

/// Maximum volume's tick labels: every hundred percent, the other ticks bare.
fn hundreds(value: f64) -> String {
    if value % VOLUME_LABEL == 0. { percent(value) } else { String::new() }
}

/// Why "Always starts at" won't be reached: it is above the maximum volume, which wins.
fn startup_above_maximum(cx: &gpui::App) -> Option<String> {
    startup_limit(settings::get(cx))
}

/// [`startup_above_maximum`] for `settings`.
fn startup_limit(settings: &Settings) -> Option<String> {
    let audio = &settings.prefs.audio;
    let above = audio.startup_volume == StartupVolume::Fixed
        && audio.startup_volume_percent > audio.max_volume_percent;
    above.then(|| tr!("Starts at {max}%, the maximum volume.", max = audio.max_volume_percent))
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

/// The Audio page's Output and Volume rows.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    SettingDef::new(
        "audio.enabled",
        OUTPUT,
        tr_noop!("Enable audio"),
        Control::Toggle,
        toggle!(prefs.audio.enabled),
    )
    .info(tr_noop!("Off, media plays silently: audio isn't decoded at all, unlike muting."))
    .applies(ApplyMode::NextMedia)
    .keywords(&["sound", "silent", "decode", "video only"]),
    SettingDef::new(
        "audio.output_device",
        OUTPUT,
        tr_noop!("Output device"),
        Control::Options(audio_devices::choices),
        text!(local.audio.output_device),
    )
    .info(tr_noop!(
        "Where sound plays. System default follows the device chosen in the system's settings; a \
         chosen device that is unplugged plays through the default until it is back."
    ))
    .keywords(&["speakers", "headphones", "bluetooth", "airpods", "hdmi"]),
    SettingDef::new(
        "audio.speaker_setup",
        OUTPUT,
        tr_noop!("Speaker setup"),
        Control::Dropdown(SpeakerSetup::CHOICES),
        choice!(prefs.audio.speaker_setup),
    )
    .info(tr_noop!(
        "Automatic plays surround sound as surround on a surround device and mixes it down for \
         stereo. Stereo always mixes it down to two channels."
    ))
    .keywords(&["channels", "surround", "5.1", "7.1", "downmix"]),
    SettingDef::new(
        "audio.stereo_mode",
        OUTPUT,
        tr_noop!("Stereo mode"),
        Control::Dropdown(StereoMode::CHOICES),
        choice!(prefs.audio.stereo_mode),
    )
    .info(tr_noop!(
        "How the left and right channels play: as they are, mixed into one (Mono), one side on \
         both speakers, swapped, Dolby Surround decoded, or surround mixed for headphones. Any but \
         Stereo wins over the speaker setup."
    ))
    .keywords(&["mono", "left", "right", "reverse", "swap", "dolby", "headphones", "channels"]),
    SettingDef::new(
        "audio.pause_on_device_disconnect",
        OUTPUT,
        tr_noop!("Pause when headphones are unplugged"),
        Control::Toggle,
        toggle!(prefs.audio.pause_on_device_disconnect),
    )
    .info(tr_noop!(
        "Pauses when the device that was playing goes away, so sound doesn't jump to speakers. Not \
         in the first seconds after playback starts, when AirPods may leave as they switch to \
         another device."
    ))
    .keywords(&["bluetooth", "disconnect", "output device"]),
    SettingDef::new(
        "audio.passthrough",
        OUTPUT,
        tr_noop!("Passthrough"),
        Control::Toggle,
        toggle!(prefs.audio.passthrough),
    )
    .info(tr_noop!(
        "Sends Dolby Digital (AC3) and DTS sound undecoded to a receiver over HDMI or S/PDIF, for \
         the receiver to decode."
    ))
    .applies(ApplyMode::Restart)
    .keywords(&["spdif", "hdmi", "ac3", "dts", "receiver", "bitstream"])
    .advanced(),
    SettingDef::new(
        "audio.max_volume_percent",
        VOLUME,
        tr_noop!("Maximum volume"),
        Control::Slider { marks: Marks::Labelled(VOLUME_TICK, hundreds), format: percent },
        number!(u32: prefs.audio.max_volume_percent),
    )
    .info(tr_noop!(
        "How far the volume goes. Above 100%, quiet media gets louder but may distort. Lowering it \
         below the current volume turns the volume down at once."
    ))
    .range(100., 300., 5.)
    .keywords(&["boost", "amplify", "louder"]),
    SettingDef::new(
        "audio.startup_volume",
        VOLUME,
        tr_noop!("Startup volume"),
        Control::Dropdown(StartupVolume::CHOICES),
        choice!(prefs.audio.startup_volume),
    )
    .info(tr_noop!(
        "The volume VLC starts at: the last one (at most 100%, so a boost doesn't carry over), or \
         always the same."
    )),
    SettingDef::new(
        "audio.startup_volume_percent",
        VOLUME,
        tr_noop!("Always starts at"),
        PERCENT,
        number!(u32: prefs.audio.startup_volume_percent),
    )
    .info(tr_noop!("The volume each launch starts at, up to the maximum volume."))
    .range(0., 300., 1.)
    .depends_on("audio.startup_volume", Condition::Is("fixed"))
    .hint(startup_above_maximum),
    SettingDef::new(
        "audio.volume_step_percent",
        VOLUME,
        tr_noop!("Volume step"),
        PERCENT,
        number!(u32: prefs.audio.volume_step_percent),
    )
    .info(tr_noop!("How much the Up and Down keys and each scroll step change the volume."))
    .range(1., 100., 1.)
    .keywords(&["increment", "arrow keys", "scroll"]),
    SettingDef::new(
        "audio.visualizer",
        VISUALIZATION,
        tr_noop!("Audio visualizer"),
        Control::Dropdown(Visualizer::CHOICES),
        choice!(prefs.audio.visualizer),
    )
    .info(tr_noop!(
        "Spectrometer, Scope and Spectrum draw the sound in the video area while media without \
         video plays. Waveform shows each file's loudness along the seek bar instead. The gear \
         button's Playback tab switches it too."
    ))
    .keywords(&["visualization", "spectrum", "scope", "waveform", "spectrometer", "music"]),
];

/// The Audio page's language rows, after the effects.
pub(in crate::settings::registry) static LANGUAGE_ENTRIES: &[SettingDef] = &[SettingDef::new(
    "audio.preferred_languages",
    TRACKS,
    tr_noop!("Audio language"),
    Control::List,
    AUDIO_LANGUAGES,
)
.info(tr_noop!(
    "Language codes such as en or fra, most preferred first, separated by commas. New media plays \
     the first one it has; empty keeps the media's own choice."
))
.validate(languages)
.applies(ApplyMode::NextMedia)
.keywords(&["dub", "track", "language"])];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_whole_hundreds_are_labelled() {
        assert_eq!(hundreds(200.), "200%");
        assert_eq!(hundreds(125.), "");
    }

    #[test]
    fn a_startup_volume_above_the_maximum_says_where_it_stops() {
        let mut settings = Settings::default();
        settings.prefs.audio.startup_volume = StartupVolume::Fixed;
        settings.prefs.audio.startup_volume_percent = 200;
        settings.prefs.audio.max_volume_percent = 150;
        let hint = startup_limit(&settings);
        assert_eq!(hint.as_deref(), Some("Starts at 150%, the maximum volume."));
        settings.prefs.audio.startup_volume = StartupVolume::RememberLast;
        assert_eq!(startup_limit(&settings), None);
    }
}
