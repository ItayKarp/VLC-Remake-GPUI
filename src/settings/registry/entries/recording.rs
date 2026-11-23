//! Recording ("Recording, Casting & Streaming Out", `recording.png`, S5.10.3): where recordings
//! go and how, finding cast devices, and what a stream sends. Recording (S8.6), casting (S8.11)
//! and streaming out (S8.12) read these when they start, so changes count from the next one;
//! "Discover cast devices" starts or stops looking at once.

use super::super::def::{Control, PathPick, SettingDef};
use super::super::value::{Choice, SettingChoice, Value, choice, text, toggle};
use crate::settings::registry::Category;
use crate::settings::{CastQuality, EncodingPreset, Tuning, expand_home};

const RECORDING: (Category, &str) = (Category::Recording, "Recording");
const CASTING: (Category, &str) = (Category::Recording, "Casting");
const STREAMING: (Category, &str) = (Category::Recording, "Streaming");

/// What the empty "Save recordings to" field shows: the folder used then.
const DEFAULT_FOLDER: &str = if cfg!(target_os = "macos") { "~/Movies" } else { "~/Videos" };

impl SettingChoice for CastQuality {
    const VALUES: &'static [Self] = &[Self::Automatic, Self::Low, Self::Medium, Self::High];
    const CHOICES: &'static [Choice] = &[
        Choice::new("automatic", "Automatic"),
        Choice::new("low", "Low"),
        Choice::new("medium", "Medium"),
        Choice::new("high", "High"),
    ];
}

impl SettingChoice for EncodingPreset {
    const VALUES: &'static [Self] =
        &[Self::Ultrafast, Self::Fast, Self::Medium, Self::Slow, Self::Veryslow];
    const CHOICES: &'static [Choice] = &[
        Choice::new("ultrafast", "Fastest"),
        Choice::new("fast", "Fast"),
        Choice::new("medium", "Medium"),
        Choice::new("slow", "Slow"),
        Choice::new("veryslow", "Smallest"),
    ];
}

impl SettingChoice for Tuning {
    const VALUES: &'static [Self] = &[
        Self::None,
        Self::Film,
        Self::Animation,
        Self::Grain,
        Self::StillImage,
        Self::ZeroLatency,
    ];
    const CHOICES: &'static [Choice] = &[
        Choice::new("none", "None"),
        Choice::new("film", "Film"),
        Choice::new("animation", "Animation"),
        Choice::new("grain", "Film grain"),
        Choice::new("still_image", "Still images"),
        Choice::new("zero_latency", "Low latency"),
    ];
}

/// A stream toggle's row.
const fn include_row(
    key: &'static str,
    label: &'static str,
    info: &'static str,
    binding: super::super::value::Binding,
) -> SettingDef {
    SettingDef::new(key, STREAMING, label, Control::Toggle, binding).info(info)
}

/// The Recording page's rows.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    SettingDef::new(
        "recordings_folder",
        RECORDING,
        "Save recordings to",
        Control::Path(PathPick::Folder),
        text!(local.recordings_folder),
    )
    .info("The folder recordings are saved in. Leave it empty for your Videos folder.")
    .placeholder(DEFAULT_FOLDER)
    .validate(existing_folder)
    .keywords(&["record", "folder", "location", "path", "output"]),
    SettingDef::new(
        "recording.keep_original_encoding",
        RECORDING,
        "Record without re-encoding when possible",
        Control::Toggle,
        toggle!(prefs.recording.keep_original_encoding),
    )
    .info(
        "Saves the stream as it arrives, which is fast and keeps its quality. Recordings are \
         always saved this way for now: converting them while recording isn't available yet.",
    )
    .keywords(&["record", "stream copy", "remux", "transcode"]),
    SettingDef::new(
        "recording.discover_cast_devices",
        CASTING,
        "Discover cast devices",
        Control::Toggle,
        toggle!(prefs.recording.discover_cast_devices),
    )
    .info(
        "Looks for Chromecasts on your network, to play on one with the player's Render \
         setting. Off, media playing on one comes back here.",
    )
    .keywords(&["chromecast", "cast", "renderer", "tv"]),
    SettingDef::new(
        "recording.cast_quality",
        CASTING,
        "Cast quality",
        Control::Dropdown(CastQuality::CHOICES),
        choice!(prefs.recording.cast_quality),
    )
    .info(
        "How media the cast device can't play is converted, from the next cast on. Lower uses \
         less of your network; Automatic is Medium.",
    )
    .keywords(&["chromecast", "cast", "bitrate", "transcode"]),
    include_row(
        "recording.play_locally_while_streaming",
        "Play locally while streaming",
        "Keeps playing here while VLC streams to the network.",
        toggle!(prefs.recording.play_locally_while_streaming),
    )
    .keywords(&["stream", "display", "monitor"]),
    include_row(
        "recording.stream_audio",
        "Include audio",
        "Sends the sound in streams.",
        toggle!(prefs.recording.stream_audio),
    )
    .keywords(&["stream", "sound"]),
    include_row(
        "recording.stream_video",
        "Include video",
        "Sends the picture in streams.",
        toggle!(prefs.recording.stream_video),
    )
    .keywords(&["stream", "picture"]),
    include_row(
        "recording.stream_subtitles",
        "Include subtitles",
        "Sends the subtitles in streams.",
        toggle!(prefs.recording.stream_subtitles),
    )
    .keywords(&["stream", "captions"]),
    SettingDef::new(
        "recording.encoding_preset",
        STREAMING,
        "Encoding preset",
        Control::Dropdown(EncodingPreset::CHOICES),
        choice!(prefs.recording.encoding_preset),
    )
    .info(
        "How hard the encoder works when it converts: faster presets use less CPU and make \
         bigger files.",
    )
    .keywords(&["x264", "preset", "transcode", "speed"]),
    SettingDef::new(
        "recording.tuning",
        STREAMING,
        "Tuning",
        Control::Dropdown(Tuning::CHOICES),
        choice!(prefs.recording.tuning),
    )
    .info("Tunes the encoder for the kind of picture, or for the least delay in live streams.")
    .keywords(&["x264", "tune", "animation", "film", "latency"]),
];

/// Checks "Save recordings to": empty, or a folder that exists.
fn existing_folder(value: &Value) -> Result<(), String> {
    let Value::Text(text) = value else { return Ok(()) };
    let text = text.trim();
    if text.is_empty() || expand_home(text).is_dir() {
        Ok(())
    } else {
        Err("There's no folder there".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use crate::settings::registry::{Value, find};

    #[test]
    fn recordings_go_to_a_folder_that_exists() {
        let folder = find("recordings_folder").expect("the row");
        let temp = std::env::temp_dir().to_string_lossy().into_owned();
        assert!(folder.accept(Value::Text(temp)).is_ok());
        assert!(folder.accept(Value::Text(String::new())).is_ok());
        assert!(folder.accept(Value::Text("~".into())).is_ok());
        let missing = std::env::temp_dir().join("rvlc-no-such-folder");
        assert!(folder.accept(Value::Text(missing.to_string_lossy().into_owned())).is_err());
    }
}
