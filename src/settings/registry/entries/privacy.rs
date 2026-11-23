//! Privacy: what VLC remembers and what may leave the machine.

use super::super::def::{Condition, Control, SettingDef};
use super::super::value::{Binding, Value, number, text, toggle};
use crate::opening::recent::MAX_COUNT;
use crate::privacy::clear;
use crate::privacy::exclusions::Exclusions;
use crate::privacy::keychain::Secret;
use crate::settings::registry::Category;

const HISTORY: (Category, &str) = (Category::Privacy, "History");
const NETWORK: (Category, &str) = (Category::Privacy, "Network");
const OPENSUBTITLES: (Category, &str) = (Category::Privacy, "OpenSubtitles");

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
        "privacy.never_remember",
        HISTORY,
        "Never remember items matching",
        Control::List,
        Binding {
            get: |s| Value::List(s.prefs.privacy.never_remember.clone()),
            set: |s, value| {
                if let Value::List(patterns) = value {
                    s.prefs.privacy.never_remember = patterns;
                }
            },
        },
    )
    .info(
        "Patterns, separated by commas, for media to leave out of the history and the recent \
         lists. * matches anything and ? one character. A name like *.part or Private matches \
         any file or folder so named; a pattern with a slash, like ~/Private/*, matches the \
         whole path or address.",
    )
    .validate(patterns)
    .keywords(&["exclude", "ignore", "blacklist", "filter", "glob", "incognito"]),
    SettingDef::new(
        "clear_history",
        HISTORY,
        "Clear history now",
        Control::Button { label: "Clear", action: clear::ask_to_clear },
        Binding::ACTION,
    )
    .info(
        "Forgets what you played, where you stopped, Open Recent and Recent Streams, after \
         saying how much there is. Your library, playlists and favorites stay.",
    )
    .keywords(&["delete", "erase", "forget", "wipe", "history"]),
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
        "privacy.redact_file_names_in_logs",
        HISTORY,
        "Hide file names in the log",
        Control::Toggle,
        toggle!(prefs.privacy.redact_file_names_in_logs),
    )
    .info(
        "Writes only the extension of each media file to the log, so a shared log doesn't show \
         what you watch.",
    )
    .keywords(&["privacy", "redact", "anonymize", "diagnostics"])
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
    SettingDef::new(
        "privacy.lastfm.submit",
        NETWORK,
        "Submit played tracks to last.fm",
        Control::Toggle,
        toggle!(prefs.privacy.lastfm.submit),
    )
    .info(
        "Sends the title, artist and album of the music you play to your last.fm profile \
         (scrobbling), using the account below.",
    )
    .keywords(&["scrobble", "scrobbling", "audioscrobbler", "lastfm", "libre.fm"]),
    SettingDef::new(
        "privacy.lastfm.username",
        NETWORK,
        "Last.fm username",
        Control::Text,
        text!(prefs.privacy.lastfm.username),
    )
    .info("The last.fm account played tracks are submitted to.")
    .depends_on("privacy.lastfm.submit", Condition::IsOn)
    .keywords(&["scrobble", "account", "login", "user name", "lastfm"]),
    SettingDef::new(
        "lastfm_password",
        NETWORK,
        "Last.fm password",
        Control::Secret(Secret::LastFmPassword),
        Binding::ACTION,
    )
    .info(
        "Kept in your system's keychain, never in VLC's settings file. Press Enter to save it; \
         Forget removes it from the keychain.",
    )
    .depends_on("privacy.lastfm.submit", Condition::IsOn)
    .keywords(&["scrobble", "account", "login", "keychain", "lastfm"]),
    SettingDef::new(
        "privacy.lastfm.submission_url",
        NETWORK,
        "Last.fm submission URL",
        Control::Text,
        text!(prefs.privacy.lastfm.submission_url),
    )
    .info(
        "Where played tracks are sent. Change it only for a service that works like last.fm, \
         such as Libre.fm.",
    )
    .validate(web_address)
    .depends_on("privacy.lastfm.submit", Condition::IsOn)
    .keywords(&["scrobble", "server", "api", "libre.fm", "lastfm"]),
    SettingDef::new(
        "privacy.opensubtitles.search",
        OPENSUBTITLES,
        "Search OpenSubtitles",
        Control::Toggle,
        toggle!(prefs.privacy.opensubtitles.search),
    )
    .info(
        "Lets Add Subtitles search OpenSubtitles.com. A search sends the video's file hash and \
         the words searched for, and a download the account below, if any.",
    )
    .keywords(&["subtitles", "download", "online", "opensubtitles.com", "consent"]),
    SettingDef::new(
        "privacy.opensubtitles.username",
        OPENSUBTITLES,
        "OpenSubtitles username",
        Control::Text,
        text!(prefs.privacy.opensubtitles.username),
    )
    .info(
        "An OpenSubtitles.com account gives more downloads a day. Without one, downloads count \
         against this computer's address.",
    )
    .depends_on("privacy.opensubtitles.search", Condition::IsOn)
    .keywords(&["subtitles", "account", "login", "user name", "quota"]),
    SettingDef::new(
        "opensubtitles_password",
        OPENSUBTITLES,
        "OpenSubtitles password",
        Control::Secret(Secret::OpenSubtitlesPassword),
        Binding::ACTION,
    )
    .info(
        "Kept in your system's keychain, never in VLC's settings file. Press Enter to save it; \
         Forget removes it from the keychain.",
    )
    .depends_on("privacy.opensubtitles.search", Condition::IsOn)
    .keywords(&["subtitles", "account", "login", "keychain", "quota"]),
];

/// Checks that every pattern of "Never remember items matching" parses.
fn patterns(value: &Value) -> Result<(), String> {
    let Value::List(patterns) = value else { return Ok(()) };
    Exclusions::parse(patterns).map(drop).map_err(|error| error.0)
}

/// Checks that the submission URL is an http or https address.
fn web_address(value: &Value) -> Result<(), String> {
    let Value::Text(text) = value else { return Ok(()) };
    match url::Url::parse(text) {
        Ok(url) if matches!(url.scheme(), "http" | "https") && url.has_host() => Ok(()),
        _ => Err("Enter an address starting with https://".to_owned()),
    }
}
