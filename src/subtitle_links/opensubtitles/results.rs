//! Reading OpenSubtitles' search answers into the results the dialog lists (S6.5.3), sorted
//! best match first, and writing the search's parameters as the API wants them.
//!
//! Best match: subtitles made for this very file (its moviehash) first, then the most
//! downloaded, then the best rated.

use std::cmp::Ordering;
use std::collections::HashSet;

use serde::Deserialize;

use super::Found;
use crate::engine::language::Language;
use crate::subtitle_links::episode;

/// One page of `GET /subtitles`.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct Page {
    data: Vec<Item>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Item {
    attributes: Attributes,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Attributes {
    language: Option<String>,
    download_count: u64,
    ratings: f32,
    votes: u64,
    release: Option<String>,
    moviehash_match: bool,
    files: Vec<File>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct File {
    file_id: u64,
    file_name: Option<String>,
}

impl Page {
    /// Each file the page lists, as a result.
    pub(super) fn into_found(self) -> Vec<Found> {
        self.data
            .into_iter()
            .flat_map(|item| {
                let attributes = item.attributes;
                let release = attributes.release.clone().unwrap_or_default();
                attributes.files.into_iter().map(move |file| Found {
                    id: file.file_id.to_string(),
                    name: file.file_name.unwrap_or_else(|| release.clone()),
                    release: release.clone(),
                    language: attributes.language.clone().unwrap_or_default(),
                    rating: (attributes.votes > 0).then_some(attributes.ratings),
                    downloads: attributes.download_count,
                    matches_file: attributes.moviehash_match,
                })
            })
            .collect()
    }
}

/// The hash search's results and the query's, each file once, best match first.
pub(super) fn merge(by_hash: Vec<Found>, by_query: Vec<Found>) -> Vec<Found> {
    let mut seen = HashSet::new();
    let mut merged: Vec<Found> = by_hash
        .into_iter()
        .map(|found| Found { matches_file: true, ..found })
        .chain(by_query)
        .filter(|found| seen.insert(found.id.clone()))
        .collect();
    merged.sort_by(best_first);
    merged
}

fn best_first(a: &Found, b: &Found) -> Ordering {
    b.matches_file
        .cmp(&a.matches_file)
        .then(b.downloads.cmp(&a.downloads))
        .then(b.rating.unwrap_or_default().total_cmp(&a.rating.unwrap_or_default()))
}

/// A search's parameters, by name.
pub(super) type Parameters = Vec<(&'static str, String)>;

/// The parameters searching for `text` (a title, its episode split off as the API asks) in
/// `languages`, sorted by name and lowercase, as the API wants to answer without a redirect.
pub(super) fn query_parameters(text: &str, languages: Option<&str>) -> Parameters {
    let (title, episode) = split_episode(text);
    let mut parameters: Parameters = vec![("query", title.to_lowercase())];
    if let Some((season, number)) = episode {
        parameters.push(("season_number", season.to_string()));
        parameters.push(("episode_number", number.to_string()));
    }
    sorted(parameters, languages)
}

/// The parameters searching by `moviehash` in `languages`.
pub(super) fn hash_parameters(moviehash: &str, languages: Option<&str>) -> Parameters {
    sorted(vec![("moviehash", moviehash.to_owned())], languages)
}

fn sorted(mut parameters: Parameters, languages: Option<&str>) -> Parameters {
    parameters.extend(languages.map(|codes| ("languages", codes.to_owned())));
    parameters.sort_by_key(|(name, _)| *name);
    parameters
}

/// `language` as OpenSubtitles names it: both Portugueses and both Chineses for those.
pub(super) fn language_codes(language: &Language) -> String {
    match language.code() {
        "pt" => "pt-br,pt-pt".to_owned(),
        "zh" => "zh-cn,zh-tw".to_owned(),
        code => code.to_lowercase(),
    }
}

/// `text`'s title and the season and episode its `S01E05` word names, if any.
fn split_episode(text: &str) -> (String, Option<(u32, u32)>) {
    let words: Vec<&str> = text.split_whitespace().collect();
    for (index, word) in words.iter().enumerate() {
        let Some(found) = episode::parse(word) else { continue };
        if let Some(season) = found.season.filter(|_| index > 0) {
            return (words[..index].join(" "), Some((season, found.first)));
        }
    }
    (text.trim().to_owned(), None)
}

/// The English name of the API's language code ("pt-BR" is "Portuguese (BR)").
pub(super) fn language_name(code: &str) -> String {
    let Some(language) = Language::parse(code) else { return code.to_owned() };
    match code.split_once('-') {
        Some((_, region)) => format!("{} ({})", language.name(), region.to_uppercase()),
        None => language.name(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(id: &str, downloads: u64, matches_file: bool) -> Found {
        Found {
            id: id.to_owned(),
            name: format!("{id}.srt"),
            release: String::new(),
            language: "en".to_owned(),
            rating: None,
            downloads,
            matches_file,
        }
    }

    #[test]
    fn hash_matches_come_first_then_downloads() {
        let merged = merge(
            vec![found("a", 5, false)],
            vec![found("b", 900, false), found("a", 5, false), found("c", 50, false)],
        );
        let ids: Vec<&str> = merged.iter().map(|found| found.id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "c"]);
        assert!(merged[0].matches_file);
    }

    #[test]
    fn an_episode_is_searched_by_its_numbers() {
        assert_eq!(
            query_parameters("The Wand S01E05", Some("en")),
            [
                ("episode_number", "5".to_owned()),
                ("languages", "en".to_owned()),
                ("query", "the wand".to_owned()),
                ("season_number", "1".to_owned()),
            ]
        );
        assert_eq!(query_parameters("Heat 1995", None), [("query", "heat 1995".to_owned())]);
    }

    #[test]
    fn a_page_lists_each_file() {
        let json = r#"{"data":[{"attributes":{"language":"pt-BR","download_count":12,
            "ratings":8.5,"votes":3,"release":"The.Wand.S01E01","moviehash_match":true,
            "files":[{"file_id":42,"file_name":"The.Wand.S01E01.srt"}]}}]}"#;
        let page: Page = serde_json::from_str(json).unwrap();
        let found = page.into_found();
        assert_eq!(found[0].id, "42");
        assert_eq!(found[0].rating, Some(8.5));
        assert!(found[0].matches_file);
        assert_eq!(language_name(&found[0].language), "Portuguese (BR)");
    }

    #[test]
    fn languages_are_named_as_the_api_does() {
        let code = |tag| language_codes(&Language::parse(tag).unwrap());
        assert_eq!(code("eng"), "en");
        assert_eq!(code("pt-BR"), "pt-br,pt-pt");
    }
}
