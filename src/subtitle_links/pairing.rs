//! Pairing a folder's (or playlist's) videos with subtitle files (S6.4.2): by episode number
//! first, then by how much of their names they share.
//!
//! Each subtitle goes to the video it fits best, so a video can get several (`E05.en.srt` and
//! `E05.he.srt`), listed in the preferred languages' order. Two files that name different
//! episodes, or different seasons, never pair; names that share too little don't either, so a
//! folder of another season's subtitles pairs nothing rather than pairing wrongly.

use std::cmp::Reverse;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::episode::{self, Episode};
use crate::engine::language::Language;
use crate::engine::sidecar;

/// The least share of the shorter name's words the other must hold for two files to pair by
/// name alone.
const CONFIDENT: f32 = 0.6;
/// What a matching episode adds to a pair's score, so it beats any pair by name alone.
const EPISODE_BONUS: f32 = 1.;
/// Words in subtitle names that say nothing about which video they belong to.
const NOISE: [&str; 5] = ["forced", "hi", "sdh", "cc", "subs"];

/// A video and the subtitles paired with it, the preferred language first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pairing {
    /// The video.
    pub video: PathBuf,
    /// Its subtitles; empty for "No match".
    pub subtitles: Vec<PathBuf>,
}

/// Pairs each of `videos` with the `subtitles` that fit it, in the videos' order. `preferred`
/// orders a video's subtitles.
pub fn pair(videos: &[PathBuf], subtitles: &[PathBuf], preferred: &[Language]) -> Vec<Pairing> {
    let videos: Vec<Name> = videos.iter().map(|path| Name::of_video(path)).collect();
    let mut pairings: Vec<Pairing> = videos
        .iter()
        .map(|video| Pairing { video: video.path.clone(), subtitles: Vec::new() })
        .collect();
    for subtitle in subtitles {
        let name = Name::of_subtitle(subtitle);
        let best = videos
            .iter()
            .enumerate()
            .filter_map(|(index, video)| Some((index, score(video, &name)?)))
            .max_by(|(a_index, a), (b_index, b)| a.total_cmp(b).then(b_index.cmp(a_index)));
        if let Some((index, _)) = best {
            pairings[index].subtitles.push(subtitle.clone());
        }
    }
    for pairing in &mut pairings {
        order_by_language(&mut pairing.subtitles, &pairing.video, preferred);
    }
    pairings
}

/// Puts `subtitles` of the video at `video` in the order of `preferred`, then by name.
pub fn order_by_language(subtitles: &mut [PathBuf], video: &Path, preferred: &[Language]) {
    subtitles.sort_by_cached_key(|path| {
        let language = sidecar::describe(path, video).language;
        let rank = language
            .and_then(|language| preferred.iter().position(|wanted| *wanted == language))
            .unwrap_or(preferred.len());
        (rank, path.clone())
    });
}

/// How well a subtitle fits a video; `None` when it doesn't.
fn score(video: &Name, subtitle: &Name) -> Option<f32> {
    let shared = shared_words(&video.words, &subtitle.words);
    match (video.episode, subtitle.episode) {
        (Some(a), Some(b)) if a.matches(b) => Some(EPISODE_BONUS + shared),
        (Some(_), Some(_)) => None,
        _ => (shared >= CONFIDENT).then_some(shared),
    }
}

/// The share of the shorter word set the other holds.
fn shared_words(a: &HashSet<String>, b: &HashSet<String>) -> f32 {
    let shorter = a.len().min(b.len());
    if shorter == 0 {
        return 0.;
    }
    a.intersection(b).count() as f32 / shorter as f32
}

/// What pairing looks at in a file name.
struct Name {
    path: PathBuf,
    episode: Option<Episode>,
    /// Its words in lower case, without languages and other subtitle-only words.
    words: HashSet<String>,
}

impl Name {
    fn of_video(path: &Path) -> Self {
        let stem = stem(path);
        let words = words(&stem).collect();
        Self { path: path.to_path_buf(), episode: episode::parse(&stem), words }
    }

    fn of_subtitle(path: &Path) -> Self {
        let stem = stem(path);
        let words = words(&stem)
            .filter(|word| !NOISE.contains(&word.as_str()) && Language::recognize(word).is_none())
            .collect();
        Self { path: path.to_path_buf(), episode: episode::parse(&stem), words }
    }
}

fn stem(path: &Path) -> String {
    path.file_stem().map_or_default(|stem| stem.to_string_lossy().into_owned())
}

/// The letters-and-digits runs of `text`, in lower case.
fn words(text: &str) -> impl Iterator<Item = String> {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .into_iter()
}

/// The candidates for one video, the ones that fit it first (best first), then the rest in
/// name order: what To This File lists (S6.4.3).
pub fn rank_for(video: &Path, subtitles: &[PathBuf]) -> Vec<PathBuf> {
    let video = Name::of_video(video);
    let mut ranked: Vec<(Option<f32>, &PathBuf)> =
        subtitles.iter().map(|path| (score(&video, &Name::of_subtitle(path)), path)).collect();
    ranked.sort_by_key(|(score, path)| {
        (Reverse(score.map(|score| (score * 1000.) as u32)), (*path).clone())
    });
    ranked.into_iter().map(|(_, path)| path.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(|name| PathBuf::from(format!("/s/{name}"))).collect()
    }

    fn names(pairing: &Pairing) -> Vec<String> {
        pairing
            .subtitles
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn episodes_pair_whatever_order_the_files_come_in() {
        let videos = paths(&["The_Wand_S01E01.mp4", "The_Wand_S01E02.mp4"]);
        let subtitles = paths(&["the.wand.1x02.srt", "The Wand S01E01.srt"]);
        let pairings = pair(&videos, &subtitles, &[]);
        assert_eq!(names(&pairings[0]), ["The Wand S01E01.srt"]);
        assert_eq!(names(&pairings[1]), ["the.wand.1x02.srt"]);
    }

    #[test]
    fn several_languages_pair_with_one_video_the_preferred_first() {
        let videos = paths(&["Show.S01E05.mkv"]);
        let subtitles = paths(&["Show.S01E05.en.srt", "Show.S01E05.he.srt"]);
        let hebrew = Language::recognize("he").unwrap();
        let pairings = pair(&videos, &subtitles, &[hebrew]);
        assert_eq!(names(&pairings[0]), ["Show.S01E05.he.srt", "Show.S01E05.en.srt"]);
    }

    #[test]
    fn another_season_pairs_nothing() {
        let videos = paths(&["Show.S02E01.mkv", "Show.S02E02.mkv"]);
        let subtitles = paths(&["Show.S01E01.srt", "Show.S01E02.srt"]);
        let pairings = pair(&videos, &subtitles, &[]);
        assert!(pairings.iter().all(|pairing| pairing.subtitles.is_empty()));
    }

    #[test]
    fn movies_pair_by_shared_words() {
        let videos = paths(&["Heat.1995.1080p.BluRay.mkv", "Alien.1979.mkv"]);
        let subtitles = paths(&["Heat 1995.English.srt", "Something Else.srt"]);
        let pairings = pair(&videos, &subtitles, &[]);
        assert_eq!(names(&pairings[0]), ["Heat 1995.English.srt"]);
        assert_eq!(pairings[1].subtitles, Vec::<PathBuf>::new());
    }

    #[test]
    fn absolute_numbers_pair_with_seasonal_ones() {
        let videos = paths(&["Show.S01E05.mkv"]);
        let pairings = pair(&videos, &paths(&["Show - 05.srt"]), &[]);
        assert_eq!(names(&pairings[0]), ["Show - 05.srt"]);
    }

    #[test]
    fn a_file_lists_its_own_subtitles_first() {
        let subtitles = paths(&["A.S01E02.srt", "Z.S01E01.srt", "B.srt"]);
        let ranked = rank_for(Path::new("/v/Z.S01E01.mkv"), &subtitles);
        assert_eq!(ranked[0], PathBuf::from("/s/Z.S01E01.srt"));
    }
}
