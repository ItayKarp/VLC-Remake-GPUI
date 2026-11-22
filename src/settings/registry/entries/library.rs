//! Library ("Playlists, Folders & Files"): opening files and folders.

use super::super::def::{ApplyMode, Control, SettingDef};
use super::super::value::{Binding, Choice, Value, toggle};
use crate::settings::registry::Category;

const OPENING: (Category, &str) = (Category::Library, "Opening Files");

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
        "library.allow_multiple_instances",
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
    SettingDef::new(
        "library.add_instead_of_replacing",
        OPENING,
        "Add to the playlist instead of replacing it",
        Control::Toggle,
        toggle!(prefs.library.add_instead_of_replacing),
    )
    .info(
        "Items you open from the library while something plays go to the end of the queue \
         instead of replacing it.",
    )
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
];
