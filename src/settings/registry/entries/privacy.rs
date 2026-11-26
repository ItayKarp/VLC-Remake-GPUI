//! Privacy: what VLC remembers and what may leave the machine.

use super::super::def::{Condition, Control, SettingDef};
use super::super::value::{Binding, Value, number, text, toggle};
use crate::opening::recent::MAX_COUNT;
use crate::privacy::clear;
use crate::privacy::exclusions::Exclusions;
use crate::privacy::keychain::Secret;
use crate::settings::registry::Category;

const HISTORY: (Category, &str) = (Category::Privacy, tr_noop!("History"));
const NETWORK: (Category, &str) = (Category::Privacy, tr_noop!("Network"));
const OPENSUBTITLES: (Category, &str) = (Category::Privacy, tr_noop!("OpenSubtitles"));

/// The Privacy page's rows.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    SettingDef::new(
        "privacy.save_recently_played",
        HISTORY,
        tr_noop!("Save recently played items"),
        Control::Toggle,
        toggle!(prefs.privacy.save_recently_played),
    )
    .info(tr_noop!(
        "Keeps a history of what you play for the library's Home and History, Open Recent and the \
         system's recent documents. Off, nothing new is written to any of them."
    ))
    .keywords(&["history", "recent files", "recents"]),
    SettingDef::new(
        "privacy.remember_playback_position",
        HISTORY,
        tr_noop!("Remember the playback position"),
        Control::Toggle,
        toggle!(prefs.privacy.remember_playback_position),
    )
    .info(tr_noop!("Saves where you stopped each file, so it can continue from there next time."))
    .depends_on("privacy.save_recently_played", Condition::IsOn)
    .keywords(&["resume", "continue watching", "bookmark"]),
    SettingDef::new(
        "privacy.never_remember",
        HISTORY,
        tr_noop!("Never remember items matching"),
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
    .info(tr_noop!(
        "Patterns, separated by commas, for media to leave out of the history and the recent \
         lists. * matches anything and ? one character. A name like *.part or Private matches any \
         file or folder so named; a pattern with a slash, like ~/Private/*, matches the whole path \
         or address."
    ))
    .validate(patterns)
    .keywords(&["exclude", "ignore", "blacklist", "filter", "glob", "incognito"]),
    SettingDef::new(
        "clear_history",
        HISTORY,
        tr_noop!("Clear history now"),
        Control::Button { label: tr_noop!("Clear"), action: clear::ask_to_clear },
        Binding::ACTION,
    )
    .info(tr_noop!(
        "Forgets what you played, where you stopped, Open Recent and Recent Streams, after saying \
         how much there is. Your library, playlists and favorites stay."
    ))
    .keywords(&["delete", "erase", "forget", "wipe", "history"]),
    SettingDef::new(
        "privacy.recent_media_count",
        HISTORY,
        tr_noop!("Items in Open Recent"),
        Control::Number { unit: "" },
        number!(u32: prefs.privacy.recent_media_count),
    )
    .info(tr_noop!("How many files and streams the Open Recent menu lists. 0 lists none."))
    .range(0., MAX_COUNT as f64, 1.)
    .depends_on("privacy.save_recently_played", Condition::IsOn)
    .keywords(&["recent files", "menu"])
    .advanced(),
    SettingDef::new(
        "privacy.redact_file_names_in_logs",
        HISTORY,
        tr_noop!("Hide file names in logs and crash reports"),
        Control::Toggle,
        toggle!(prefs.privacy.redact_file_names_in_logs),
    )
    .info(tr_noop!(
        "Writes only the extension of each media file to the log, and leaves file names and \
         folders out of crash reports, so a shared log or report doesn't show what you watch."
    ))
    .keywords(&["privacy", "redact", "anonymize", "diagnostics", "crash report"])
    .advanced(),
    SettingDef::new(
        "privacy.fetch_online_metadata",
        NETWORK,
        tr_noop!("Fetch titles and cover art from the internet"),
        Control::Toggle,
        toggle!(prefs.privacy.fetch_online_metadata),
    )
    .info(tr_noop!(
        "Allows looking up titles and cover art online for media without its own. VLC has no \
         online source for them yet, so nothing about your files leaves this computer either way."
    ))
    .hint(no_online_source_hint)
    .keywords(&["artwork", "album art", "metadata", "online"]),
    SettingDef::new(
        "privacy.lastfm.submit",
        NETWORK,
        tr_noop!("Submit played tracks to last.fm"),
        Control::Toggle,
        toggle!(prefs.privacy.lastfm.submit),
    )
    .info(tr_noop!(
        "Sends the title, artist and album of the music you play to your last.fm profile \
         (scrobbling), using the account below."
    ))
    .keywords(&["scrobble", "scrobbling", "audioscrobbler", "lastfm", "libre.fm"]),
    SettingDef::new(
        "privacy.lastfm.username",
        NETWORK,
        tr_noop!("Last.fm username"),
        Control::Text,
        text!(prefs.privacy.lastfm.username),
    )
    .info(tr_noop!("The last.fm account played tracks are submitted to."))
    .depends_on("privacy.lastfm.submit", Condition::IsOn)
    .keywords(&["scrobble", "account", "login", "user name", "lastfm"]),
    SettingDef::new(
        "lastfm_password",
        NETWORK,
        tr_noop!("Last.fm password"),
        Control::Secret(Secret::LastFmPassword),
        Binding::ACTION,
    )
    .info(tr_noop!(
        "Kept in your system's keychain, never in VLC's settings file. Press Enter to save it; \
         Forget removes it from the keychain."
    ))
    .depends_on("privacy.lastfm.submit", Condition::IsOn)
    .keywords(&["scrobble", "account", "login", "keychain", "lastfm"]),
    SettingDef::new(
        "privacy.lastfm.submission_url",
        NETWORK,
        tr_noop!("Last.fm submission URL"),
        Control::Text,
        text!(prefs.privacy.lastfm.submission_url),
    )
    .info(tr_noop!(
        "Where played tracks are sent. Change it only for a service that works like last.fm, such \
         as Libre.fm."
    ))
    .validate(web_address)
    .depends_on("privacy.lastfm.submit", Condition::IsOn)
    .keywords(&["scrobble", "server", "api", "libre.fm", "lastfm"]),
    SettingDef::new(
        "privacy.opensubtitles.search",
        OPENSUBTITLES,
        tr_noop!("Search OpenSubtitles"),
        Control::Toggle,
        toggle!(prefs.privacy.opensubtitles.search),
    )
    .info(tr_noop!(
        "Lets Add Subtitles search OpenSubtitles.com. A search sends the video's file hash and the \
         words searched for, and a download the account below, if any."
    ))
    .keywords(&["subtitles", "download", "online", "opensubtitles.com", "consent"]),
    SettingDef::new(
        "privacy.opensubtitles.username",
        OPENSUBTITLES,
        tr_noop!("OpenSubtitles username"),
        Control::Text,
        text!(prefs.privacy.opensubtitles.username),
    )
    .info(tr_noop!(
        "An OpenSubtitles.com account gives more downloads a day. Without one, downloads count \
         against this computer's address."
    ))
    .depends_on("privacy.opensubtitles.search", Condition::IsOn)
    .keywords(&["subtitles", "account", "login", "user name", "quota"]),
    SettingDef::new(
        "opensubtitles_password",
        OPENSUBTITLES,
        tr_noop!("OpenSubtitles password"),
        Control::Secret(Secret::OpenSubtitlesPassword),
        Binding::ACTION,
    )
    .info(tr_noop!(
        "Kept in your system's keychain, never in VLC's settings file. Press Enter to save it; \
         Forget removes it from the keychain."
    ))
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
        _ => Err(tr!("Enter an address starting with https://").to_owned()),
    }
}

/// Says, while the switch is on, that it changes nothing yet: libVLC 3 looks titles and art up
/// online only through Lua scripts, and none ship (S9.9.4, `docs/legal.md`).
fn no_online_source_hint(cx: &gpui::App) -> Option<String> {
    crate::settings::get(cx).prefs.privacy.fetch_online_metadata.then(|| {
        tr!("VLC has no online source yet, so titles and art come from the file and its folder.")
            .to_owned()
    })
}
