//! Subtitles' Appearance section (S5.8.3, `subtitles_01.png`; live since S6.2): whether styled
//! files keep their own look, where subtitles sit, and the font, size, colors, outline and
//! shadow of plain-text subtitles. Subtitle files the app draws change at once; subtitles inside
//! the video, which libVLC draws with the style it read when the player started, take them at
//! the next launch.

use std::sync::OnceLock;

use super::super::def::{Condition, Control, Marks, SettingDef};
use super::super::value::{Choice, SettingChoice, choice, number, text, toggle};
use crate::engine::subtitle_style::{Outline, RelativeSize};
use crate::settings::registry::{Category, hex_color};
use crate::settings::{self, SizeValues, SubtitleArea};

const APPEARANCE: (Category, &str) = (Category::Subtitles, "Appearance");
/// Text opacity's ticks, and the ones labelled.
const OPACITY_TICK: f64 = 10.;
const OPACITY_LABEL: f64 = 50.;
/// When a change shows, said on the rows that have nothing else to say.
const LIVE_NOTE: &str =
    "Subtitle files change at once; subtitles inside the video when VLC starts again.";

impl SettingChoice for SubtitleArea {
    const VALUES: &'static [Self] = &[Self::Picture, Self::Bars];
    const CHOICES: &'static [Choice] =
        &[Choice::new("picture", "On the picture"), Choice::new("bars", "In the black bars")];
}

impl SettingChoice for SizeValues {
    const VALUES: &'static [Self] = &[Self::Relative, Self::Pixels];
    const CHOICES: &'static [Choice] =
        &[Choice::new("relative", "Relative"), Choice::new("pixels", "Pixels")];
}

impl SettingChoice for RelativeSize {
    const VALUES: &'static [Self] =
        &[Self::Automatic, Self::Smaller, Self::Small, Self::Normal, Self::Large, Self::Larger];
    const CHOICES: &'static [Choice] = &[
        Choice::new("automatic", "Automatic"),
        Choice::new("smaller", "Smaller"),
        Choice::new("small", "Small"),
        Choice::new("normal", "Normal"),
        Choice::new("large", "Large"),
        Choice::new("larger", "Larger"),
    ];
}

impl SettingChoice for Outline {
    const VALUES: &'static [Self] = &[Self::None, Self::Thin, Self::Normal, Self::Thick];
    const CHOICES: &'static [Choice] = &[
        Choice::new("none", "None"),
        Choice::new("thin", "Thin"),
        Choice::new("normal", "Normal"),
        Choice::new("thick", "Thick"),
    ];
}

/// An opacity as the slider says it: "50%".
fn percent(value: f64) -> String {
    format!("{value:.0}%")
}

/// Text opacity's tick labels: 0%, 50% and 100%, the other ticks bare.
fn halves(value: f64) -> String {
    if value % OPACITY_LABEL == 0. { percent(value) } else { String::new() }
}

/// The font families installed here, lowercased, read once.
fn installed_fonts(cx: &gpui::App) -> &'static [String] {
    static FONTS: OnceLock<Vec<String>> = OnceLock::new();
    FONTS.get_or_init(|| {
        cx.text_system().all_font_names().iter().map(|name| name.to_lowercase()).collect()
    })
}

/// Warns that the chosen font isn't on this machine (settings synced from another one).
fn font_missing(cx: &gpui::App) -> Option<String> {
    let font = settings::get(cx).prefs.subtitles.font.trim().to_lowercase();
    missing(&font, installed_fonts(cx)).then(|| {
        "Not installed on this computer: subtitles use the system font instead.".to_owned()
    })
}

/// Whether `font` (lowercased; empty for the system's) is a family `installed` lacks.
fn missing(font: &str, installed: &[String]) -> bool {
    !font.is_empty() && !installed.iter().any(|name| name == font)
}

/// Warns that subtitles can't be read in the chosen colors: fully transparent text, or an
/// outline in the text's own color.
fn unreadable(cx: &gpui::App) -> Option<String> {
    let subtitles = &settings::get(cx).prefs.subtitles;
    if subtitles.text_opacity_percent == 0 {
        return Some("Subtitles will be invisible.".to_owned());
    }
    let same = hex_color(&subtitles.text_color) == hex_color(&subtitles.outline_color);
    (same && subtitles.outline != Outline::None)
        .then(|| "The outline is the text's color, so letters run together.".to_owned())
}

/// The Subtitles page's Appearance rows.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    SettingDef::new(
        "subtitles.file_styles",
        APPEARANCE,
        "Embedded Sub Format",
        Control::Toggle,
        toggle!(prefs.subtitles.file_styles),
    )
    .info(
        "On, styled (ASS/SSA) subtitle files keep the fonts, colors and positions they give \
         themselves; off, they are drawn in the style below.",
    )
    .keywords(&["ass", "ssa", "styles", "embedded"]),
    SettingDef::new(
        "subtitles.area",
        APPEARANCE,
        "Subtitle area",
        Control::Segmented(SubtitleArea::CHOICES),
        choice!(prefs.subtitles.area),
    )
    .info("In the black bars, subtitle files sit below the picture when it has bars there.")
    .keywords(&["position", "letterbox", "placement"]),
    SettingDef::new(
        "subtitles.font",
        APPEARANCE,
        "Font",
        Control::Text,
        text!(prefs.subtitles.font),
    )
    .info("The font family subtitles are drawn in, such as Helvetica; empty uses the system's.")
    .placeholder("System, sans-serif")
    .hint(font_missing)
    .keywords(&["typeface", "family"]),
    SettingDef::new(
        "subtitles.size_values",
        APPEARANCE,
        "Size values",
        Control::Segmented(SizeValues::CHOICES),
        choice!(prefs.subtitles.size_values),
    )
    .info("Relative sizes grow with the video; Pixels keeps the text the same height."),
    SettingDef::new(
        "subtitles.relative_size",
        APPEARANCE,
        "Font size",
        Control::Dropdown(RelativeSize::CHOICES),
        choice!(prefs.subtitles.relative_size),
    )
    .info(LIVE_NOTE)
    .depends_on("subtitles.size_values", Condition::Is("relative"))
    .keywords(&["text size", "bigger", "smaller"]),
    SettingDef::new(
        "subtitles.pixel_size",
        APPEARANCE,
        "Font size in pixels",
        Control::Number { unit: "px" },
        number!(u32: prefs.subtitles.pixel_size),
    )
    .info("The text's height in the video's pixels, while Size values is Pixels.")
    .range(8., 200., 1.)
    .depends_on("subtitles.size_values", Condition::Is("pixels"))
    .keywords(&["text size"]),
    SettingDef::new(
        "subtitles.bold",
        APPEARANCE,
        "Bold",
        Control::Toggle,
        toggle!(prefs.subtitles.bold),
    )
    .info(LIVE_NOTE),
    SettingDef::new(
        "subtitles.text_color",
        APPEARANCE,
        "Text color",
        Control::HexColor,
        text!(prefs.subtitles.text_color),
    )
    .info("A color such as #FFFFFF, #FF0 or yellow; the square opens a palette.")
    .hint(unreadable)
    .keywords(&["colour", "font color"]),
    SettingDef::new(
        "subtitles.text_opacity_percent",
        APPEARANCE,
        "Text opacity",
        Control::Slider { marks: Marks::Labelled(OPACITY_TICK, halves), format: percent },
        number!(u32: prefs.subtitles.text_opacity_percent),
    )
    .info(LIVE_NOTE)
    .hint(unreadable)
    .range(0., 100., 1.)
    .keywords(&["transparency", "see-through"]),
    SettingDef::new(
        "subtitles.outline",
        APPEARANCE,
        "Outline thickness",
        Control::Dropdown(Outline::CHOICES),
        choice!(prefs.subtitles.outline),
    )
    .info("The border around each letter that keeps text readable on bright pictures.")
    .keywords(&["border", "stroke", "edge"]),
    SettingDef::new(
        "subtitles.outline_color",
        APPEARANCE,
        "Outline color",
        Control::HexColor,
        text!(prefs.subtitles.outline_color),
    )
    .info("A color such as #000000, #000 or black; the square opens a palette.")
    .depends_on("subtitles.outline", Condition::IsNot("none"))
    .keywords(&["colour", "border"]),
    SettingDef::new(
        "subtitles.shadow",
        APPEARANCE,
        "Shadow",
        Control::Toggle,
        toggle!(prefs.subtitles.shadow),
    )
    .info("A soft shadow under the text.")
    .keywords(&["drop shadow"]),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_named_font_that_is_not_installed_is_missing() {
        let installed = ["helvetica".to_owned(), "avenir next".to_owned()];
        assert!(!missing("", &installed));
        assert!(!missing("avenir next", &installed));
        assert!(missing("comic neue", &installed));
    }

    #[test]
    fn opacity_ticks_are_labelled_at_halves() {
        assert_eq!(halves(50.), "50%");
        assert_eq!(halves(30.), "");
    }
}
