//! Privacy settings (Settings → Privacy, S5.5): what VLC remembers, and what may leave the
//! machine.

use serde::{Deserialize, Serialize};

use super::super::lenient::lenient;

/// How many items Open Recent keeps by default.
const DEFAULT_RECENT_MEDIA_COUNT: u32 = 10;
/// Where scrobbles go unless the user names a compatible service (Libre.fm, a self-hosted one).
pub const DEFAULT_SUBMISSION_URL: &str = "https://ws.audioscrobbler.com/2.0/";

/// Privacy preferences (Stage 5).
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent switches, each its own row in Settings → Privacy"
)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrivacySettings {
    /// "Save recently played items" (S5.5.1). Also governs the OS recent documents
    /// list (S0.6.5) and the playback history (S1.7): off, nothing is written to either.
    #[serde(deserialize_with = "lenient")]
    pub save_recently_played: bool,
    /// Hides media file names in the log file, keeping only their extensions
    /// (S0.7.1). The home directory is always replaced by `~`.
    #[serde(deserialize_with = "lenient")]
    pub redact_file_names_in_logs: bool,
    /// How many items Open Recent keeps (S1.3.5); `0` keeps none.
    #[serde(deserialize_with = "lenient")]
    pub recent_media_count: u32,
    /// "Remember the playback position" (S5.5.1): save where media was left, so it can resume
    /// (S1.7). Plays are still counted while history is saved.
    #[serde(deserialize_with = "lenient")]
    pub remember_playback_position: bool,
    /// "Never remember items matching" (S5.5.1): glob patterns (`privacy::exclusions`); media
    /// matching any of them is left out of the history and the recent lists.
    #[serde(deserialize_with = "lenient")]
    pub never_remember: Vec<String>,
    /// "Fetch titles and cover art from the internet" (S5.5, S4.3.4): whether the library may
    /// look covers up online for media without one. Off by default: nothing about the user's
    /// files leaves the machine unless they turn it on.
    #[serde(deserialize_with = "lenient")]
    pub fetch_online_metadata: bool,
    /// "Submit played tracks to last.fm" and its account (S5.5.2).
    #[serde(deserialize_with = "lenient")]
    pub lastfm: LastFmSettings,
    /// "Search OpenSubtitles" and its account (S6.5).
    #[serde(deserialize_with = "lenient")]
    pub opensubtitles: OpenSubtitlesSettings,
}

impl Default for PrivacySettings {
    fn default() -> Self {
        PrivacySettings {
            save_recently_played: true,
            redact_file_names_in_logs: false,
            recent_media_count: DEFAULT_RECENT_MEDIA_COUNT,
            remember_playback_position: true,
            never_remember: Vec::new(),
            fetch_online_metadata: false,
            lastfm: LastFmSettings::default(),
            opensubtitles: OpenSubtitlesSettings::default(),
        }
    }
}

/// The last.fm account played tracks are submitted to (S5.5.2). The password is never here: it
/// lives in the OS keychain (`privacy::keychain`, S5.5.3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LastFmSettings {
    /// "Submit played tracks to last.fm". Off by default.
    #[serde(deserialize_with = "lenient")]
    pub submit: bool,
    /// "Last.fm username".
    #[serde(deserialize_with = "lenient")]
    pub username: String,
    /// "Last.fm submission URL": the scrobbling API's address.
    #[serde(deserialize_with = "lenient")]
    pub submission_url: String,
}

impl Default for LastFmSettings {
    fn default() -> Self {
        Self {
            submit: false,
            username: String::new(),
            submission_url: DEFAULT_SUBMISSION_URL.to_owned(),
        }
    }
}

/// OpenSubtitles.com (S6.5): whether the Add Subtitles dialog may search it, and the account
/// downloads count against. The password and the account's token live in the OS keychain.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OpenSubtitlesSettings {
    /// "Search OpenSubtitles" (S6.5.4): the consent to send a video's file hash and name there.
    /// Off until the user allows it, the first time they search or here.
    #[serde(deserialize_with = "lenient")]
    pub search: bool,
    /// "OpenSubtitles username": empty downloads without an account.
    #[serde(deserialize_with = "lenient")]
    pub username: String,
}
