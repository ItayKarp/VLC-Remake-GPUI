//! Library ("Playlists, Folders & Files", `library.png`, S5.9): the media folders, opening files
//! and folders, and where titles and artwork come from.
//!
//! The Media Folders list isn't in the design (S5.9.3): it tops the page, as D11 recommends.

use super::super::def::{ApplyMode, Condition, Control, SettingDef};
use super::super::value::{Binding, Choice, SettingChoice, Value, choice, toggle};
use crate::settings::registry::Category;
use crate::settings::{self, ArtworkSource};

const FOLDERS: (Category, &str) = (Category::Library, "Media Folders");
const OPENING: (Category, &str) = (Category::Library, "Opening Files");
const METADATA: (Category, &str) = (Category::Library, "Metadata & Artwork");

/// The key of "Only allow one VLC window", which its dependents name.
const ONE_WINDOW_KEY: &str = "library.allow_multiple_instances";
/// The key of "Use the media library", which the folder list depends on.
const ENABLED_KEY: &str = "library.enabled";

impl SettingChoice for ArtworkSource {
    const VALUES: &'static [Self] = &[Self::File, Self::Folder, Self::Internet];
    const CHOICES: &'static [Choice] = &[
        Choice::new("file", "From the file only"),
        Choice::new("folder", "From the file and its folder"),
        Choice::new("internet", "Also from the internet"),
    ];
}

/// Says when "Also from the internet" can't use the internet: Privacy's switch is off.
fn internet_hint(cx: &gpui::App) -> Option<String> {
    let prefs = &settings::get(cx).prefs;
    (prefs.library.artwork_source == ArtworkSource::Internet
        && !prefs.privacy.fetch_online_metadata)
        .then(|| {
            "The internet is off in Settings → Privacy, so only the file and its folder are used."
                .to_owned()
        })
}

/// "When opening a folder"'s choices, over the `include_subfolders` switch.
const FOLDER_CHOICES: &[Choice] = &[
    Choice::new("include_subfolders", "Include subfolders"),
    Choice::new("folder_only", "This folder only"),
];

/// "Only allow one VLC window": the opposite of `allow_multiple_instances`.
const ONE_WINDOW: Binding = Binding {
    get: |s| Value::Bool(!s.prefs.library.allow_multiple_instances),
    set: |s, value| {
        if let Value::Bool(one) = value {
            s.prefs.library.allow_multiple_instances = !one;
        }
    },
};

/// "When opening a folder" as a choice over `include_subfolders`.
const OPENING_FOLDERS: Binding = Binding {
    get: |s| {
        let include = s.prefs.library.include_subfolders;
        Value::Choice(if include { "include_subfolders" } else { "folder_only" })
    },
    set: |s, value| {
        if let Value::Choice(id) = value {
            s.prefs.library.include_subfolders = id == "include_subfolders";
        }
    },
};

/// The Library page's rows.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    SettingDef::new(
        "library.media_folders",
        FOLDERS,
        "Media folders",
        Control::MediaFolders,
        Binding::ACTION,
    )
    .info("The folders the library shows and keeps up to date as files change.")
    .depends_on(ENABLED_KEY, Condition::IsOn)
    .keywords(&["library folders", "add folder", "remove folder", "rescan", "resync", "index"]),
    SettingDef::new(
        ONE_WINDOW_KEY,
        OPENING,
        "Only allow one VLC window",
        Control::Toggle,
        ONE_WINDOW,
    )
    .info(
        "Files you open while VLC is running go to the window already open. Off, each one opens \
         its own VLC.",
    )
    .applies(ApplyMode::Restart)
    .keywords(&["single instance", "one instance", "multiple windows"]),
    // On macOS the system itself hands files opened in the Finder to the running app.
    #[cfg(not(target_os = "macos"))]
    SettingDef::new(
        "library.one_window_from_file_manager",
        OPENING,
        "Apply to files opened from the file manager",
        Control::Toggle,
        toggle!(prefs.library.one_window_from_file_manager),
    )
    .info(
        "Files you open from the file manager go to the window already open too. Off, each one \
         opens its own VLC.",
    )
    .depends_on(ONE_WINDOW_KEY, Condition::IsOn)
    .keywords(&["explorer", "finder", "double-click", "single instance"]),
    SettingDef::new(
        "library.add_instead_of_replacing",
        OPENING,
        "Add to the playlist instead of replacing it",
        Control::Toggle,
        toggle!(prefs.library.add_instead_of_replacing),
    )
    .info(
        "Files you open while VLC is running, and items you open from the library while \
         something plays, go to the end of the queue instead of replacing it.",
    )
    .depends_on(ONE_WINDOW_KEY, Condition::IsOn)
    .keywords(&["enqueue", "queue", "append"]),
    SettingDef::new(
        "library.include_subfolders",
        OPENING,
        "When opening a folder",
        Control::Dropdown(FOLDER_CHOICES),
        OPENING_FOLDERS,
    )
    .info("Whether opening a folder also plays the media in the folders inside it.")
    .keywords(&["subfolders", "recursive", "directory"]),
    SettingDef::new(
        "library.show_hidden_files",
        OPENING,
        "Show hidden files",
        Control::Toggle,
        toggle!(prefs.library.show_hidden_files),
    )
    .info("Lists files whose names start with a dot, and files the system marks hidden.")
    .keywords(&["dot files", "system files"]),
    SettingDef::new(
        "library.artwork_source",
        METADATA,
        "Fetch titles and artwork",
        Control::Dropdown(ArtworkSource::CHOICES),
        choice!(prefs.library.artwork_source),
    )
    .info(
        "Where the library finds titles and covers: the file's own tags and pictures, a cover \
         image in its folder, or the internet, while Settings → Privacy allows it.",
    )
    .hint(internet_hint)
    .keywords(&["metadata", "cover art", "album art", "tags", "online"]),
    SettingDef::new(
        ENABLED_KEY,
        METADATA,
        "Use the media library",
        Control::Toggle,
        toggle!(prefs.library.enabled),
    )
    .info(
        "Shows the library and keeps its folders indexed. Off, the library is hidden and \
         nothing is indexed; VLC offers to keep what was indexed for later.",
    )
    .keywords(&["index", "indexing", "scan", "file browser", "sidebar"]),
];
