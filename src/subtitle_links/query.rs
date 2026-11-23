//! The OpenSubtitles search a video starts with (S6.4.4): its title as a person would type it,
//! read from the file name. `The_Wand_S01E01.mp4` searches "The Wand S01E01", and
//! `Heat.1995.1080p.BluRay.mkv` "Heat 1995".

use std::path::Path;

use super::episode;

/// Words that start a release's technical details: everything from one on is left out.
const RELEASE_WORDS: [&str; 22] = [
    "480p", "576p", "720p", "1080p", "1080i", "2160p", "4k", "uhd", "bluray", "bdrip", "brrip",
    "webrip", "webdl", "web", "hdtv", "dvdrip", "x264", "x265", "h264", "h265", "hevc", "remux",
];
/// The years a word may be, kept as the end of a film's title.
const YEARS: std::ops::RangeInclusive<u32> = 1900..=2099;

/// The search for the video at `path`.
pub fn for_video(path: &Path) -> String {
    let stem = path.file_stem().map_or_default(|stem| stem.to_string_lossy().into_owned());
    let words: Vec<&str> = stem
        .split(|character: char| character == '_' || character == '.' || character.is_whitespace())
        .filter(|word| !word.is_empty() && !is_bracketed(word))
        .collect();
    let mut title: Vec<String> = Vec::new();
    for word in words {
        let lower = word.to_ascii_lowercase();
        if RELEASE_WORDS.contains(&lower.as_str()) {
            break;
        }
        if let Some(found) = episode::parse(word).filter(|found| found.season.is_some()) {
            title.push(format!("S{:02}E{:02}", found.season.unwrap_or_default(), found.first));
            break;
        }
        let year = lower.parse::<u32>().is_ok_and(|number| YEARS.contains(&number));
        title.push(word.to_owned());
        if year && title.len() > 1 {
            break;
        }
    }
    title.join(" ")
}

/// A release group's tag (`[Group]`), left out.
fn is_bracketed(word: &str) -> bool {
    word.starts_with('[') && word.ends_with(']')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(name: &str) -> String {
        for_video(Path::new(name))
    }

    #[test]
    fn an_episode_ends_the_title() {
        assert_eq!(query("/v/The_Wand_S01E01.mp4"), "The Wand S01E01");
        assert_eq!(query("the.wand.s01e01.720p.webrip.mkv"), "the wand S01E01");
        assert_eq!(query("The Wand 1x05 Pilot.mkv"), "The Wand S01E05");
    }

    #[test]
    fn a_film_keeps_its_year_and_loses_the_release() {
        assert_eq!(query("Heat.1995.1080p.BluRay.mkv"), "Heat 1995");
        assert_eq!(query("[Group] Alien.mkv"), "Alien");
        assert_eq!(query("2001.A.Space.Odyssey.1968.mkv"), "2001 A Space Odyssey 1968");
    }
}
