//! Subtitles ("Subtitle Defaults"): whether subtitles show and in which language.

use super::super::def::{ApplyMode, Condition, Control, SettingDef};
use super::super::value::{Binding, Value, toggle};
use super::languages;
use crate::settings::registry::Category;

const TRACKS: (Category, &str) = (Category::Subtitles, "Tracks & Languages");

/// "Preferred language": subtitle languages, most preferred first.
const SUBTITLE_LANGUAGES: Binding = Binding {
    get: |s| Value::List(s.prefs.subtitles.preferred_languages.clone()),
    set: |s, value| {
        if let Value::List(languages) = value {
            s.prefs.subtitles.preferred_languages = languages;
        }
    },
};

/// The Subtitles page's rows.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    SettingDef::new(
        "subtitles.enabled",
        TRACKS,
        "Enable subtitles",
        Control::Toggle,
        toggle!(prefs.subtitles.enabled),
    )
    .info(
        "Shows subtitles in a preferred language, or the media's own default ones. Off, only \
         forced subtitles (signs and foreign dialogue) show.",
    )
    .applies(ApplyMode::NextMedia)
    .keywords(&["captions", "cc", "subs"]),
    SettingDef::new(
        "subtitles.preferred_languages",
        TRACKS,
        "Preferred language",
        Control::List,
        SUBTITLE_LANGUAGES,
    )
    .info(
        "Language codes such as en or heb, most preferred first, separated by commas. New media \
         shows the first one it has.",
    )
    .validate(languages)
    .depends_on("subtitles.enabled", Condition::IsOn)
    .applies(ApplyMode::NextMedia)
    .keywords(&["captions", "subs", "language"]),
];
