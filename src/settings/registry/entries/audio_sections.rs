//! Audio → Effects' Compressor, Spatializer and Stereo widening with their values (S7.9), the
//! rows the player's settings panel shows as its Audio tab's sections (D24). Each slider uses
//! libVLC's range; the Spatializer's values are on a 0–10 scale, and the Stereo Widener's gains
//! are real percentages. All are heard at once.

use super::super::def::{Condition, Control, Marks, SettingDef};
use super::super::value::{Binding, number, toggle};
use crate::effects::{self, EffectSection};
use crate::settings::registry::Category;

const EFFECTS: (Category, &str) = (Category::Audio, "Effects");
const COMPRESSOR: &str = "audio.compressor.enabled";
const SPATIALIZER: &str = "audio.spatializer.enabled";
const WIDENER: &str = "audio.stereo_widener.enabled";

/// A value with one decimal: "8.0".
fn tenths(value: f64) -> String {
    format!("{value:.1}")
}

fn milliseconds(value: f64) -> String {
    format!("{value:.1} ms")
}

fn decibels(value: f64) -> String {
    format!("{value:.1} dB")
}

fn ratio(value: f64) -> String {
    format!("{value:.1} : 1")
}

fn percent(value: f64) -> String {
    format!("{value:.0} %")
}

/// RMS / Peak's mix, as libVLC's 0–1: "0.2".
fn mix(value: f64) -> String {
    super::super::rules::trim((value * 100.).round() / 100.)
}

/// Says when the player's panel changed `section` for this session (D24).
fn overridden(cx: &gpui::App, section: &EffectSection) -> Option<String> {
    effects::any_changed(cx, section).then(|| {
        "The player's settings panel changed these for this session; its Save as Default \
         writes them here."
            .to_owned()
    })
}

/// A slider under the switch `parent`.
const fn slider(
    key: &'static str,
    label: &'static str,
    (format, binding): (fn(f64) -> String, Binding),
    (min, max, step): (f64, f64, f64),
    (parent, info): (&'static str, &'static str),
) -> SettingDef {
    let ticks = if max - min <= 10. { Marks::Ticks(1.) } else { Marks::None };
    SettingDef::new(key, EFFECTS, label, Control::Slider { marks: ticks, format }, binding)
        .info(info)
        .range(min, max, step)
        .depends_on(parent, Condition::IsOn)
}

/// A gain of the Stereo Widener, 0 to `max` percent, ticked every 10%.
const fn widener_gain(
    key: &'static str,
    label: &'static str,
    binding: Binding,
    max: f64,
    info: &'static str,
) -> SettingDef {
    let control = Control::Slider { marks: Marks::Ticks(10.), format: percent };
    SettingDef::new(key, EFFECTS, label, control, binding)
        .info(info)
        .range(0., max, 1.)
        .depends_on(WIDENER, Condition::IsOn)
}

/// The rows, the Compressor first.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    SettingDef::new(
        COMPRESSOR,
        EFFECTS,
        "Compressor",
        Control::Toggle,
        toggle!(prefs.audio.compressor.enabled),
    )
    .info(
        "Turns sound above the threshold down, so loud and quiet passages sit closer together. \
         Wins over Night mode while on.",
    )
    .hint(|cx| overridden(cx, &effects::AUDIO_COMPRESSOR))
    .keywords(&["dynamic range", "dynamics", "loud", "limiter"]),
    slider(
        "audio.compressor.rms_peak",
        "RMS / Peak",
        (mix, number!(f32: prefs.audio.compressor.rms_peak)),
        (0., 1., 0.01),
        (COMPRESSOR, "How the level is measured: 0 follows peaks, 1 the average (RMS)."),
    ),
    slider(
        "audio.compressor.attack",
        "Attack",
        (milliseconds, number!(f32: prefs.audio.compressor.attack_ms)),
        (1.5, 400., 0.5),
        (COMPRESSOR, "How fast loud sound is turned down."),
    ),
    slider(
        "audio.compressor.release",
        "Release",
        (milliseconds, number!(f32: prefs.audio.compressor.release_ms)),
        (2., 800., 1.),
        (COMPRESSOR, "How fast the compressor lets go once sound is quieter again."),
    ),
    slider(
        "audio.compressor.threshold",
        "Threshold",
        (decibels, number!(f32: prefs.audio.compressor.threshold_db)),
        (-30., 0., 0.5),
        (COMPRESSOR, "Sound louder than this is turned down."),
    ),
    slider(
        "audio.compressor.ratio",
        "Ratio",
        (ratio, number!(f32: prefs.audio.compressor.ratio)),
        (1., 20., 0.1),
        (COMPRESSOR, "How much: at 4 : 1, sound 4 dB over the threshold comes out 1 dB over."),
    ),
    slider(
        "audio.compressor.knee",
        "Knee Radius",
        (decibels, number!(f32: prefs.audio.compressor.knee_db)),
        (1., 10., 0.1),
        (COMPRESSOR, "How gently the compressor starts around the threshold."),
    ),
    slider(
        "audio.compressor.makeup_gain",
        "Makeup Gain",
        (decibels, number!(f32: prefs.audio.compressor.makeup_db)),
        (0., 24., 0.1),
        (COMPRESSOR, "Raises everything after, to win back the level the compressor took."),
    ),
    SettingDef::new(
        SPATIALIZER,
        EFFECTS,
        "Spatializer",
        Control::Toggle,
        toggle!(prefs.audio.spatializer.enabled),
    )
    .info("Plays sound as if in a room around you, with the room's reverberation.")
    .hint(|cx| overridden(cx, &effects::AUDIO_SPATIALIZER))
    .keywords(&["reverb", "room", "space", "echo"]),
    slider(
        "audio.spatializer.size",
        "Size",
        (tenths, number!(f32: prefs.audio.spatializer.size)),
        (0., 10., 0.1),
        (SPATIALIZER, "The room's size: larger rooms ring longer."),
    ),
    slider(
        "audio.spatializer.width",
        "Width",
        (tenths, number!(f32: prefs.audio.spatializer.width)),
        (0., 10., 0.1),
        (SPATIALIZER, "How wide the reverberation spreads between left and right."),
    ),
    slider(
        "audio.spatializer.wet",
        "Wet",
        (tenths, number!(f32: prefs.audio.spatializer.wet)),
        (0., 10., 0.1),
        (SPATIALIZER, "How loud the reverberation is."),
    ),
    slider(
        "audio.spatializer.dry",
        "Dry",
        (tenths, number!(f32: prefs.audio.spatializer.dry)),
        (0., 10., 0.1),
        (SPATIALIZER, "How loud the original sound is."),
    ),
    slider(
        "audio.spatializer.damp",
        "Damp",
        (tenths, number!(f32: prefs.audio.spatializer.damp)),
        (0., 10., 0.1),
        (SPATIALIZER, "How much the walls soften high frequencies."),
    ),
    SettingDef::new(
        WIDENER,
        EFFECTS,
        "Stereo widening",
        Control::Toggle,
        toggle!(prefs.audio.stereo_widener.enabled),
    )
    .info("Makes stereo sound wider than the speakers or headphones are apart. Mono has none.")
    .hint(|cx| overridden(cx, &effects::AUDIO_STEREO_WIDENER))
    .keywords(&["stereo", "wide", "spread", "widener"]),
    slider(
        "audio.stereo_widener.delay",
        "Delay Time",
        (milliseconds, number!(f32: prefs.audio.stereo_widener.delay_ms)),
        (1., 100., 0.5),
        (WIDENER, "How late each channel's copy reaches the other side."),
    ),
    widener_gain(
        "audio.stereo_widener.feedback",
        "Feedback Gain",
        number!(f32: prefs.audio.stereo_widener.feedback_percent),
        90.,
        "How much of the copy is fed back into itself.",
    ),
    widener_gain(
        "audio.stereo_widener.crossfeed",
        "Crossfeed",
        number!(f32: prefs.audio.stereo_widener.crossfeed_percent),
        80.,
        "How loud each channel's copy is on the other side.",
    ),
    widener_gain(
        "audio.stereo_widener.dry_mix",
        "Dry Mix",
        number!(f32: prefs.audio.stereo_widener.dry_mix_percent),
        100.,
        "How loud the original sound is.",
    ),
];
