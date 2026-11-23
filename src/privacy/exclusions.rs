//! "Never remember items matching" (S5.5.1): media matching one of the user's glob patterns
//! ([`glob`](super::glob)) is left out of the playback history, Open Recent, the OS recent
//! documents and Recent Streams.
//!
//! A pattern with a `/` (or `\`) is a location pattern and must match the whole path or URL:
//! `~/Private/*` leaves out everything under that folder, `*` crossing folders. Any other pattern
//! is a name pattern and needs to match only one part of it: a file or folder name of a path, or
//! the host or one path segment of a URL. So `*.part` leaves out partial downloads anywhere and
//! `Private` everything inside any folder named so.

use std::path::Path;

use gpui::App;

use super::glob::{Glob, GlobError};
use crate::app::media::Media;
use crate::settings;

/// The user's patterns, parsed.
#[derive(Clone, Debug, Default)]
pub struct Exclusions {
    patterns: Vec<Pattern>,
}

#[derive(Clone, Debug)]
enum Pattern {
    /// Matches the whole location.
    Location(Glob),
    /// Matches one part of it.
    Name(Glob),
}

impl Exclusions {
    /// Parses every pattern, or says why the first bad one is refused (the row's check).
    pub fn parse(patterns: &[String]) -> Result<Self, GlobError> {
        let patterns = patterns.iter().map(|pattern| Pattern::parse(pattern));
        Ok(Self { patterns: patterns.collect::<Result<_, _>>()? })
    }

    /// The saved patterns. One that doesn't parse (the settings file was edited by hand) is
    /// skipped whole, never applied in part.
    pub fn from_settings(cx: &App) -> Self {
        let saved = &settings::get(cx).prefs.privacy.never_remember;
        let parsed = saved.iter().filter_map(|pattern| match Pattern::parse(pattern) {
            Ok(pattern) => Some(pattern),
            Err(error) => {
                tracing::warn!("privacy: skipping a pattern: {}", error.0);
                None
            }
        });
        Self { patterns: parsed.collect() }
    }

    /// Whether `media` must not be remembered.
    pub fn excludes(&self, media: &Media) -> bool {
        self.patterns.iter().any(|pattern| pattern.matches(media))
    }

    /// Whether the URL `url` must not be remembered.
    pub fn excludes_url(&self, url: &str) -> bool {
        self.excludes(&Media::Url(url.to_owned()))
    }
}

/// Whether the saved patterns leave `media` out.
pub fn excluded(media: &Media, cx: &App) -> bool {
    let saved = &settings::get(cx).prefs.privacy.never_remember;
    !saved.is_empty() && Exclusions::from_settings(cx).excludes(media)
}

impl Pattern {
    fn parse(pattern: &str) -> Result<Self, GlobError> {
        let pattern = pattern.trim();
        if pattern.contains(['/', '\\']) {
            Glob::parse(&expand_home(&slashes(pattern))).map(Self::Location)
        } else {
            Glob::parse(pattern).map(Self::Name)
        }
    }

    fn matches(&self, media: &Media) -> bool {
        match (self, media) {
            (Self::Location(glob), Media::File(path)) => glob.matches(&path_text(path)),
            (Self::Location(glob), Media::Url(url)) => glob.matches(url),
            (Self::Location(glob), Media::Device(device)) => glob.matches(device.mrl()),
            (Self::Name(glob), Media::File(path)) => {
                path.iter().any(|part| glob.matches(&part.to_string_lossy()))
            }
            (Self::Name(glob), Media::Url(url)) => url_parts(url).any(|part| glob.matches(part)),
            (Self::Name(glob), Media::Device(device)) => glob.matches(&device.display_name()),
        }
    }
}

/// `text` with `\` as `/`, so one pattern works for paths from every OS.
fn slashes(text: &str) -> String {
    text.replace('\\', "/")
}

/// `path` as location patterns see it.
fn path_text(path: &Path) -> String {
    slashes(&path.to_string_lossy())
}

/// A leading `~` as the home folder.
fn expand_home(pattern: &str) -> String {
    let home = dirs::home_dir().map(|home| path_text(&home));
    match (pattern.strip_prefix('~'), home) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with('/') => home + rest,
        _ => pattern.to_owned(),
    }
}

/// A URL's host and path segments, which name patterns match one of.
fn url_parts(url: &str) -> impl Iterator<Item = &str> {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let path = rest.split(['?', '#']).next().unwrap_or(rest);
    path.split('/').filter(|part| !part.is_empty())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn exclusions(patterns: &[&str]) -> Exclusions {
        let patterns: Vec<String> = patterns.iter().map(|&p| p.to_owned()).collect();
        Exclusions::parse(&patterns).expect("valid patterns")
    }

    fn file(path: &str) -> Media {
        Media::File(PathBuf::from(path))
    }

    #[test]
    fn name_patterns_match_any_part_of_a_path() {
        let rules = exclusions(&["*.part", "Private"]);
        assert!(rules.excludes(&file("/media/films/Film.mkv.part")));
        assert!(rules.excludes(&file("/media/private/holiday.mp4")));
        assert!(!rules.excludes(&file("/media/films/Film.mkv")));
        assert!(!rules.excludes(&file("/media/privately/a.mp4")));
    }

    #[test]
    fn location_patterns_match_the_whole_path() {
        let rules = exclusions(&["/media/kids/*"]);
        assert!(rules.excludes(&file("/media/kids/cartoons/a.mkv")));
        assert!(!rules.excludes(&file("/other/media/kids/a.mkv")));
    }

    #[test]
    fn home_is_expanded() {
        let Some(home) = dirs::home_dir() else { return };
        let rules = exclusions(&["~/Secret/*"]);
        assert!(rules.excludes(&Media::File(home.join("Secret").join("a.mp4"))));
        assert!(!rules.excludes(&Media::File(home.join("Public").join("a.mp4"))));
    }

    #[test]
    fn urls_match_by_host_segment_or_whole() {
        let rules = exclusions(&["*.example.com"]);
        assert!(rules.excludes_url("https://cdn.example.com/live/stream.m3u8?token=1"));
        assert!(!rules.excludes_url("https://example.org/live"));
        let whole = exclusions(&["rtsp://192.168.1.*"]);
        assert!(whole.excludes_url("rtsp://192.168.1.20/cam"));
    }

    #[test]
    fn a_bad_pattern_refuses_the_list() {
        let patterns = vec!["*.part".to_owned(), "ep[1-".to_owned()];
        assert!(Exclusions::parse(&patterns).is_err());
    }
}
