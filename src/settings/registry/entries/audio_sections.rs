//! Audio → Effects' Compressor, Spatializer and Stereo widening with their values (S7.9), the
//! rows the player's settings panel shows as its Audio tab's sections (D24). Each slider uses
//! libVLC's range; the Spatializer's values are on a 0–10 scale, and the Stereo Widener's gains
//! are real percentages. All are heard at once.

use super::super::def::{Condition, Control, Marks, SettingDef};
use super::super::value::{Binding, number, toggle};
use crate::effects::{self, EffectSection};
use crate::settings::registry::Category;

const EFFECTS: (Category, &str) = (Category::Audio, tr_noop!("Effects"));
const COMPRESSOR: &str = "audio.compressor.enabled";
const SPATIALIZER: &str = "audio.spatializer.enabled";
const WIDENER: &str = "audio.stereo_widener.enabled";

/// A value with one decimal: "8.0".
fn tenths(value: f64) -> String {
    crate::i18n::decimal(value, 1)
}

fn milliseconds(value: f64) -> String {
    tr!("{value} ms", value = crate::i18n::decimal(value, 1))
}

fn decibels(value: f64) -> String {
    tr!("{value} dB", value = crate::i18n::decimal(value, 1))
}

fn ratio(value: f64) -> String {
    tr!("{value} : 1", value = crate::i18n::decimal(value, 1))
}

fn percent(value: f64) -> String {
    tr!("{value} %", value = crate::i18n::decimal(value, 0))
}

/// RMS / Peak's mix, as libVLC's 0–1: "0.2".
fn mix(value: f64) -> String {
    super::super::rules::trim((value * 100.).round() / 100.)
}

/// Says when the player's panel changed `section` for this session (D24).
fn overridden(cx: &gpui::App, section: &EffectSection) -> Option<String> {
    effects::any_changed(cx, section).then(|| {
        tr!("The player's settings panel changed these for this session; its Save as Default \
             writes them here.")
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
        tr_noop!("Compressor"),
        Control::Toggle,
        toggle!(prefs.audio.compressor.enabled),
    )
    .info(tr_noop!(
        "Turns sound above the threshold down, so loud and quiet passages sit closer together. \
         Wins over Night mode while on."
    ))
    .hint(|cx| overridden(cx, &effects::AUDIO_COMPRESSOR))
    .keywords(&["dynamic range", "dynamics", "loud", "limiter"]),
    slider(
        "audio.compressor.rms_peak",
        tr_noop!("RMS / Peak"),
        (mix, number!(f32: prefs.audio.compressor.rms_peak)),
        (0., 1., 0.01),
        (COMPRESSOR, tr_noop!("How the level is measured: 0 follows peaks, 1 the average (RMS).")),
    ),
    slider(
        "audio.compressor.attack",
        tr_noop!("Attack"),
        (milliseconds, number!(f32: prefs.audio.compressor.attack_ms)),
        (1.5, 400., 0.5),
        (COMPRESSOR, tr_noop!("How fast loud sound is turned down.")),
    ),
    slider(
        "audio.compressor.release",
        tr_noop!("Release"),
        (milliseconds, number!(f32: prefs.audio.compressor.release_ms)),
        (2., 800., 1.),
        (COMPRESSOR, tr_noop!("How fast the compressor lets go once sound is quieter again.")),
    ),
    slider(
        "audio.compressor.threshold",
        tr_noop!("Threshold"),
        (decibels, number!(f32: prefs.audio.compressor.threshold_db)),
        (-30., 0., 0.5),
        (COMPRESSOR, tr_noop!("Sound louder than this is turned down.")),
    ),
    slider(
        "audio.compressor.ratio",
        tr_noop!("Ratio"),
        (ratio, number!(f32: prefs.audio.compressor.ratio)),
        (1., 20., 0.1),
        (
            COMPRESSOR,
            tr_noop!("How much: at 4 : 1, sound 4 dB over the threshold comes out 1 dB over."),
        ),
    ),
    slider(
        "audio.compressor.knee",
        tr_noop!("Knee Radius"),
        (decibels, number!(f32: prefs.audio.compressor.knee_db)),
        (1., 10., 0.1),
        (COMPRESSOR, tr_noop!("How gently the compressor starts around the threshold.")),
    ),
    slider(
        "audio.compressor.makeup_gain",
        tr_noop!("Makeup Gain"),
        (decibels, number!(f32: prefs.audio.compressor.makeup_db)),
        (0., 24., 0.1),
        (
            COMPRESSOR,
            tr_noop!("Raises everything after, to win back the level the compressor took."),
        ),
    ),
    SettingDef::new(
        SPATIALIZER,
        EFFECTS,
        tr_noop!("Spatializer"),
        Control::Toggle,
        toggle!(prefs.audio.spatializer.enabled),
    )
    .info(tr_noop!("Plays sound as if in a room around you, with the room's reverberation."))
    .hint(|cx| overridden(cx, &effects::AUDIO_SPATIALIZER))
    .keywords(&["reverb", "room", "space", "echo"]),
    slider(
        "audio.spatializer.size",
        tr_noop!("Size"),
        (tenths, number!(f32: prefs.audio.spatializer.size)),
        (0., 10., 0.1),
        (SPATIALIZER, tr_noop!("The room's size: larger rooms ring longer.")),
    ),
    slider(
        "audio.spatializer.width",
        tr_noop!("Width"),
        (tenths, number!(f32: prefs.audio.spatializer.width)),
        (0., 10., 0.1),
        (SPATIALIZER, tr_noop!("How wide the reverberation spreads between left and right.")),
    ),
    slider(
        "audio.spatializer.wet",
        tr_noop!("Wet"),
        (tenths, number!(f32: prefs.audio.spatializer.wet)),
        (0., 10., 0.1),
        (SPATIALIZER, tr_noop!("How loud the reverberation is.")),
    ),
    slider(
        "audio.spatializer.dry",
        tr_noop!("Dry"),
        (tenths, number!(f32: prefs.audio.spatializer.dry)),
        (0., 10., 0.1),
        (SPATIALIZER, tr_noop!("How loud the original sound is.")),
    ),
    slider(
        "audio.spatializer.damp",
        tr_noop!("Damp"),
        (tenths, number!(f32: prefs.audio.spatializer.damp)),
        (0., 10., 0.1),
        (SPATIALIZER, tr_noop!("How much the walls soften high frequencies.")),
    ),
    SettingDef::new(
        WIDENER,
        EFFECTS,
        tr_noop!("Stereo widening"),
        Control::Toggle,
        toggle!(prefs.audio.stereo_widener.enabled),
    )
    .info(tr_noop!(
        "Makes stereo sound wider than the speakers or headphones are apart. Mono has none."
    ))
    .hint(|cx| overridden(cx, &effects::AUDIO_STEREO_WIDENER))
    .keywords(&["stereo", "wide", "spread", "widener"]),
    slider(
        "audio.stereo_widener.delay",
        tr_noop!("Delay Time"),
        (milliseconds, number!(f32: prefs.audio.stereo_widener.delay_ms)),
        (1., 100., 0.5),
        (WIDENER, tr_noop!("How late each channel's copy reaches the other side.")),
    ),
    widener_gain(
        "audio.stereo_widener.feedback",
        tr_noop!("Feedback Gain"),
        number!(f32: prefs.audio.stereo_widener.feedback_percent),
        90.,
        tr_noop!("How much of the copy is fed back into itself."),
    ),
    widener_gain(
        "audio.stereo_widener.crossfeed",
        tr_noop!("Crossfeed"),
        number!(f32: prefs.audio.stereo_widener.crossfeed_percent),
        80.,
        tr_noop!("How loud each channel's copy is on the other side."),
    ),
    widener_gain(
        "audio.stereo_widener.dry_mix",
        tr_noop!("Dry Mix"),
        number!(f32: prefs.audio.stereo_widener.dry_mix_percent),
        100.,
        tr_noop!("How loud the original sound is."),
    ),
];
