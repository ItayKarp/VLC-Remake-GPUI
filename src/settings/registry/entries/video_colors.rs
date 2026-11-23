//! Video → Colors (S7.4): the color filters every video shows through, each off by default as
//! in VLC. The player's settings panel changes them for the session only (D24).
//!
//! Each filter has its own switch under "Use color filters", and its values depend on that
//! switch (S7.4.3), but for the two colors, which can be picked before their filter is on. Sepia has no switch: its intensity is off at 0, like the picture's
//! enhancements (S7.3).

use super::super::def::{Condition, Control, Marks, SettingDef};
use super::super::value::{Binding, Choice, SettingChoice, choice, number, text, toggle};
use crate::engine::colors::GradientMode;
use crate::settings::registry::Category;

const COLORS: (Category, &str) = (Category::Video, "Colors");
/// The switch every filter depends on.
const ENABLED: &str = "video.colors.enabled";
const THRESHOLD: &str = "video.colors.threshold";
const POSTERIZE: &str = "video.colors.posterize";
const GRADIENT: &str = "video.colors.gradient";
/// What the rows change, and how they relate to the player's panel (D24).
const COLORS_INFO: &str = "Seen at once, and kept for every video. The player's settings panel \
                           can change them for the rest of the session.";
/// The words every row is found by.
const KEYWORDS: &[&str] = &["color", "filter", "effect"];

impl SettingChoice for GradientMode {
    const VALUES: &'static [Self] = &[Self::Gradient, Self::Edge, Self::Hough];
    const CHOICES: &'static [Choice] = &[
        Choice::new("gradient", "Gradient"),
        Choice::new("edge", "Edge"),
        Choice::new("hough", "Hough"),
    ];
}

/// A whole number as its slider says it: "20".
fn whole(value: f64) -> String {
    format!("{value:.0}")
}

/// Sepia's intensity as its slider says it: "Off" at 0, else "120".
fn intensity(value: f64) -> String {
    if value > 0. { whole(value) } else { "Off".to_owned() }
}

/// Says when the player's panel has changed the colors for this session, so this page's values
/// aren't what plays (D24).
fn colors_overridden(cx: &gpui::App) -> Option<String> {
    crate::effects::any_changed(cx, &crate::effects::VIDEO_COLORS).then(|| {
        "The player's settings panel changed these for this session; its Save as Default \
         writes them here."
            .to_owned()
    })
}

/// A row under `parent`'s switch.
const fn row(
    key: &'static str,
    label: &'static str,
    control: (Control, Binding),
    parent: &'static str,
    info: &'static str,
) -> SettingDef {
    SettingDef::new(key, COLORS, label, control.0, control.1)
        .info(info)
        .depends_on(parent, Condition::IsOn)
        .keywords(KEYWORDS)
}

/// A filter's switch, under "Use color filters".
const fn filter(
    key: &'static str,
    label: &'static str,
    binding: Binding,
    info: &'static str,
) -> SettingDef {
    row(key, label, (Control::Toggle, binding), ENABLED, info)
}

/// A slider from 0 to 255 under `parent`'s switch.
const fn byte_slider(
    key: &'static str,
    label: &'static str,
    binding: Binding,
    parent: &'static str,
    info: &'static str,
) -> SettingDef {
    let slider = Control::Slider { marks: Marks::None, format: whole };
    row(key, label, (slider, binding), parent, info).range(0., 255., 1.)
}

/// The Colors rows.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    SettingDef::new(
        ENABLED,
        COLORS,
        "Use color filters",
        Control::Toggle,
        toggle!(prefs.video.colors.enabled),
    )
    .info(COLORS_INFO)
    .hint(colors_overridden)
    .keywords(KEYWORDS),
    filter(
        "video.colors.extract",
        "Color Extraction",
        toggle!(prefs.video.colors.extract),
        "Keeps one color and turns the rest of the picture grey. Seen at once, kept for every \
         video.",
    ),
    row(
        "video.colors.extract_color",
        "Extracted Color",
        (Control::HexColor, text!(prefs.video.colors.extract_color)),
        ENABLED,
        "The color Color Extraction keeps: #RRGGBB, #RGB or a name.",
    )
    .placeholder("#FF0000"),
    filter(
        THRESHOLD,
        "Color Threshold",
        toggle!(prefs.video.colors.threshold),
        "Keeps the colors near one and turns the rest grey. Seen at once, kept for every video.",
    ),
    row(
        "video.colors.threshold_color",
        "Threshold Color",
        (Control::HexColor, text!(prefs.video.colors.threshold_color)),
        ENABLED,
        "The color Color Threshold keeps colors near: #RRGGBB, #RGB or a name.",
    )
    .placeholder("#FF0000"),
    byte_slider(
        "video.colors.threshold_saturation",
        "Saturation",
        number!(u32: prefs.video.colors.threshold_saturation),
        THRESHOLD,
        "How saturated a color must be for Color Threshold to keep it; greyer ones go grey.",
    ),
    byte_slider(
        "video.colors.threshold_similarity",
        "Similarity",
        number!(u32: prefs.video.colors.threshold_similarity),
        THRESHOLD,
        "How far from the threshold color a color may be and still be kept.",
    ),
    filter(
        "video.colors.negate",
        "Negate Colors",
        toggle!(prefs.video.colors.negate),
        "Shows the picture as its negative. Seen at once, kept for every video.",
    )
    .keywords(&["invert", "negative", "color", "filter"]),
    filter(
        POSTERIZE,
        "Posterize",
        toggle!(prefs.video.colors.posterize),
        "Reduces the picture to a few flat colors, like a poster. Seen at once, kept for every \
         video.",
    ),
    row(
        "video.colors.posterize_levels",
        "Levels",
        (
            Control::Slider { marks: Marks::None, format: whole },
            number!(u32: prefs.video.colors.posterize_levels),
        ),
        POSTERIZE,
        "How many shades each of red, green and blue keeps: fewer is flatter.",
    )
    .range(2., 256., 1.),
    filter(
        GRADIENT,
        "Gradient",
        toggle!(prefs.video.colors.gradient),
        "Draws where the picture changes. Seen at once, kept for every video.",
    )
    .keywords(&["edge", "outline", "sketch", "color", "filter"]),
    row(
        "video.colors.gradient_mode",
        "Mode",
        (Control::Dropdown(GradientMode::CHOICES), choice!(prefs.video.colors.gradient_mode)),
        GRADIENT,
        "Gradient draws how fast brightness changes, Edge only the edges, Hough the circles it \
         finds, which is slow.",
    ),
    row(
        "video.colors.gradient_color",
        "Color",
        (Control::Toggle, toggle!(prefs.video.colors.gradient_color)),
        GRADIENT,
        "Draws in the picture's own colors rather than in white.",
    ),
    row(
        "video.colors.cartoon",
        "Cartoon",
        (Control::Toggle, toggle!(prefs.video.colors.cartoon)),
        GRADIENT,
        "Draws the picture as a cartoon, flat colors outlined in black; Gradient and Edge only.",
    ),
    row(
        "video.colors.sepia",
        "Sepia Intensity",
        (
            Control::Slider { marks: Marks::None, format: intensity },
            number!(u32: prefs.video.colors.sepia),
        ),
        ENABLED,
        "Tints the picture brown like an old photograph; 0 is off. Seen at once, kept for every \
         video.",
    )
    .range(0., 255., 1.)
    .keywords(&["old", "vintage", "brown", "color", "filter"]),
];
