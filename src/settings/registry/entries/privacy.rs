//! Privacy: what VLC remembers and what may leave the machine.

use super::super::def::{Condition, Control, SettingDef};
use super::super::value::{number, toggle};
use crate::opening::recent::MAX_COUNT;
use crate::settings::registry::Category;

const HISTORY: (Category, &str) = (Category::Privacy, "History");
const NETWORK: (Category, &str) = (Category::Privacy, "Network");

/// The Privacy page's rows.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    SettingDef::new(
        "privacy.save_recently_played",
        HISTORY,
        "Save recently played items",
        Control::Toggle,
        toggle!(prefs.privacy.save_recently_played),
    )
    .info(
        "Keeps a history of what you play for the library's Home and History, Open Recent and \
         the system's recent documents. Off, nothing new is written to any of them.",
    )
    .keywords(&["history", "recent files", "recents"]),
    SettingDef::new(
        "privacy.remember_playback_position",
        HISTORY,
        "Remember the playback position",
        Control::Toggle,
        toggle!(prefs.privacy.remember_playback_position),
    )
    .info("Saves where you stopped each file, so it can continue from there next time.")
    .depends_on("privacy.save_recently_played", Condition::IsOn)
    .keywords(&["resume", "continue watching", "bookmark"]),
    SettingDef::new(
        "privacy.recent_media_count",
        HISTORY,
        "Items in Open Recent",
        Control::Number { unit: "" },
        number!(u32: prefs.privacy.recent_media_count),
    )
    .info("How many files and streams the Open Recent menu lists. 0 lists none.")
    .range(0., MAX_COUNT as f64, 1.)
    .depends_on("privacy.save_recently_played", Condition::IsOn)
    .keywords(&["recent files", "menu"])
    .advanced(),
    SettingDef::new(
        "privacy.fetch_online_metadata",
        NETWORK,
        "Fetch titles and cover art from the internet",
        Control::Toggle,
        toggle!(prefs.privacy.fetch_online_metadata),
    )
    .info(
        "Looks up cover art online for media without its own. Off, nothing about your files \
         leaves this computer.",
    )
    .keywords(&["artwork", "album art", "metadata", "online"]),
];
