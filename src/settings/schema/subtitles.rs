//! Subtitle settings (Settings → Subtitles, "Tracks & Languages"; S1.6.3, rows in S5.8.3).

use serde::{Deserialize, Serialize};

use super::super::lenient::lenient;

/// Subtitle preferences that follow the user between machines.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SubtitleSettings {
    /// "Enable subtitles": new media shows subtitles in a preferred language, or the media's
    /// default ones. Off, only forced subtitles in the audio's language show.
    #[serde(deserialize_with = "lenient")]
    pub enabled: bool,
    /// "Preferred language": language codes (`en`, `heb`), most preferred first.
    #[serde(deserialize_with = "lenient")]
    pub preferred_languages: Vec<String>,
}

impl Default for SubtitleSettings {
    fn default() -> Self {
        Self { enabled: true, preferred_languages: Vec::new() }
    }
}
