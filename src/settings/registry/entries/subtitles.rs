//! Subtitles ("Subtitle Defaults", S5.8.3): whether subtitles show and in which language, the
//! text encoding, and which subtitle files load with a video. Appearance is in
//! `subtitle_style.rs`.

use super::super::def::{ApplyMode, Condition, Control, SettingDef};
use super::super::value::{Binding, Choice, SettingChoice, Value, choice, toggle};
use super::languages;
use crate::engine::subtitle_style::FileMatching;
use crate::settings::registry::Category;

const TRACKS: (Category, &str) = (Category::Subtitles, "Tracks & Languages");
const LOADING: (Category, &str) = (Category::Subtitles, "Loading Subtitles");
/// The Text encoding choice that guesses, stored as an empty encoding.
const AUTOMATIC: &str = "auto";

/// "Text encoding": libVLC's character set names, and what each is for; the first is Automatic.
/// The video menu's Text Encoding offers the same for one file (S6.1.6).
pub(crate) static ENCODINGS: &[Choice] = &[
    Choice::new(AUTOMATIC, "Automatic"),
    Choice::new("UTF-8", "Unicode (UTF-8)"),
    Choice::new("UTF-16", "Unicode (UTF-16)"),
    Choice::new("Windows-1252", "Western European (Windows-1252)"),
    Choice::new("ISO-8859-15", "Western European (ISO-8859-15)"),
    Choice::new("Windows-1250", "Central European (Windows-1250)"),
    Choice::new("Windows-1251", "Cyrillic (Windows-1251)"),
    Choice::new("KOI8-R", "Russian (KOI8-R)"),
    Choice::new("Windows-1253", "Greek (Windows-1253)"),
    Choice::new("Windows-1254", "Turkish (Windows-1254)"),
    Choice::new("Windows-1255", "Hebrew (Windows-1255)"),
    Choice::new("Windows-1256", "Arabic (Windows-1256)"),
    Choice::new("Windows-1257", "Baltic (Windows-1257)"),
    Choice::new("Windows-1258", "Vietnamese (Windows-1258)"),
    Choice::new("Windows-874", "Thai (Windows-874)"),
    Choice::new("GB18030", "Simplified Chinese (GB18030)"),
    Choice::new("Big5", "Traditional Chinese (Big5)"),
    Choice::new("Shift_JIS", "Japanese (Shift JIS)"),
    Choice::new("EUC-KR", "Korean (EUC-KR)"),
];

impl SettingChoice for FileMatching {
    const VALUES: &'static [Self] =
        &[Self::Exact, Self::TitleWithExtra, Self::ContainingTitle, Self::AnyInFolder];
    const CHOICES: &'static [Choice] = &[
        Choice::new("exact", "Exact name"),
        Choice::new("title_with_extra", "Title with extra characters"),
        Choice::new("containing_title", "Any name containing the title"),
        Choice::new("any_in_folder", "Any subtitle in the folder"),
    ];
}

/// "Preferred language": subtitle languages, most preferred first.
const SUBTITLE_LANGUAGES: Binding = Binding {
    get: |s| Value::List(s.prefs.subtitles.preferred_languages.clone()),
    set: |s, value| {
        if let Value::List(languages) = value {
            s.prefs.subtitles.preferred_languages = languages;
        }
    },
};

/// "Text encoding": the stored character set as its choice, an unknown one as Automatic.
const TEXT_ENCODING: Binding = Binding {
    get: |s| {
        let stored = &s.prefs.subtitles.text_encoding;
        let listed = ENCODINGS.iter().skip(1).find(|choice| choice.id == stored.as_str());
        Value::Choice(listed.map_or(AUTOMATIC, |choice| choice.id))
    },
    set: |s, value| {
        if let Value::Choice(id) = value {
            let encoding = if id == AUTOMATIC { "" } else { id };
            encoding.clone_into(&mut s.prefs.subtitles.text_encoding);
        }
    },
};

/// The Subtitles page's Tracks & Languages and Loading Subtitles rows.
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
    SettingDef::new(
        "subtitles.text_encoding",
        TRACKS,
        "Text encoding",
        Control::Dropdown(ENCODINGS),
        TEXT_ENCODING,
    )
    .info(
        "The character set text subtitles (SRT, SUB) are written in. Automatic reads Unicode \
         and guesses the rest; pick one when letters show as gibberish.",
    )
    .applies(ApplyMode::NextMedia)
    .keywords(&["charset", "codepage", "gibberish", "mojibake", "hebrew", "cyrillic"]),
    SettingDef::new(
        "subtitles.load_files",
        LOADING,
        "Load subtitle files automatically",
        Control::Toggle,
        toggle!(prefs.subtitles.load_files),
    )
    .info(
        "Loads subtitle files next to a video, and in a Subs or Subtitles folder beside it, as \
         File name matching says. A language in the name (Movie.en.srt) counts as the track's.",
    )
    .applies(ApplyMode::NextMedia)
    .keywords(&["srt", "autoload", "external", "folder"]),
    SettingDef::new(
        "subtitles.file_matching",
        LOADING,
        "File name matching",
        Control::Dropdown(FileMatching::CHOICES),
        choice!(prefs.subtitles.file_matching),
    )
    .info(
        "Which files load: for Movie.mkv, Exact name takes Movie.srt, and Title with extra \
         characters also Movie.en.srt.",
    )
    .depends_on("subtitles.load_files", Condition::IsOn)
    .applies(ApplyMode::NextMedia)
    .keywords(&["srt", "fuzzy", "name"]),
];
