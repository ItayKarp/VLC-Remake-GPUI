//! Searching the library (S4.7.5): what a typed query means, and the cards of the media it
//! finds through the index ([`crate::db::search`]).
//!
//! A query is trimmed and cut to [`MAX_QUERY_CHARS`], then split at spaces into words; a word
//! of punctuation alone is dropped, so a query of nothing but spaces and punctuation searches
//! nothing. Every word must appear somewhere in a file's name, title, artist or album, ignoring
//! case and accents. Words of three characters or more go to the index; shorter ones (a trigram
//! needs three) are checked here against each hit's text, folded the same way.
//!
//! A media with files in several places is found once. Results come by title, A to Z, and
//! stop at [`MAX_RESULTS`].

use std::collections::HashSet;
use std::path::Path;

use icu_normalizer::DecomposingNormalizerBorrowed;
use rusqlite::Connection;

use super::display;
use super::folder_view::{Card, kind_by_extension};
use super::sort::sort_cards;
use super::watch_state::WatchState;
use crate::db::metadata::MediaKind;
use crate::db::search::{self, Hit};
use crate::history::policy::WatchedRule;
use crate::navigation::sort::Sort;

/// The longest query searched, in characters; the rest is ignored.
pub const MAX_QUERY_CHARS: usize = 200;
/// The most cards a search lists.
pub const MAX_RESULTS: usize = 1000;
/// The most files the index hands back to be checked: enough for every file of a big library
/// when only short words were typed and the index can't narrow them.
const MAX_HITS: usize = 200_000;
/// The shortest word the trigram index can match.
const TRIGRAM: usize = 3;

/// What a query looks for: its words, folded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Query {
    words: Vec<String>,
}

impl Query {
    /// The query `typed` means, or `None` when it looks for nothing.
    pub fn parse(typed: &str) -> Option<Self> {
        let words: Vec<String> = clean(typed)
            .split_whitespace()
            .filter(|word| word.chars().any(char::is_alphanumeric))
            .map(fold)
            .collect();
        (!words.is_empty()).then_some(Self { words })
    }

    /// The index's query for the long words (FTS5 syntax, each a quoted phrase); `None` when
    /// every word is short.
    fn expression(&self) -> Option<String> {
        let phrases: Vec<String> = self
            .words
            .iter()
            .filter(|word| !is_short(word))
            .map(|word| format!("\"{}\"", word.replace('"', "\"\"")))
            .collect();
        (!phrases.is_empty()).then(|| phrases.join(" AND "))
    }

    /// Whether `text` holds every short word.
    fn matches_short_words(&self, text: &str) -> bool {
        let text = fold(text);
        self.words.iter().filter(|word| is_short(word)).all(|word| text.contains(word.as_str()))
    }
}

/// `typed`, trimmed and cut to [`MAX_QUERY_CHARS`]: what a search route keeps.
pub fn clean(typed: &str) -> String {
    typed.trim().chars().take(MAX_QUERY_CHARS).collect::<String>().trim_end().to_owned()
}

/// `text` as searches compare it: accents removed and lowercase.
pub fn fold(text: &str) -> String {
    let decomposed = DecomposingNormalizerBorrowed::new_nfd().normalize(text);
    decomposed.chars().filter(|c| !is_combining(*c)).flat_map(char::to_lowercase).collect()
}

fn is_short(word: &str) -> bool {
    word.chars().count() < TRIGRAM
}

/// Whether `c` is a combining diacritical mark, which [`fold`] drops after decomposing.
fn is_combining(c: char) -> bool {
    matches!(
        c,
        '\u{0300}'..='\u{036F}'
            | '\u{1AB0}'..='\u{1AFF}'
            | '\u{1DC0}'..='\u{1DFF}'
            | '\u{20D0}'..='\u{20FF}'
            | '\u{FE20}'..='\u{FE2F}'
    )
}

/// The cards of the media `query` finds, of the `wanted` kind (any when `None`), by title, with
/// `rule` deciding what is watched. Blocks on the database.
pub fn find(
    connection: &Connection,
    query: &Query,
    wanted: Option<MediaKind>,
    rule: WatchedRule,
) -> rusqlite::Result<Vec<Card>> {
    let hits = search::find(connection, query.expression().as_deref(), MAX_HITS)?;
    let mut seen = HashSet::new();
    let mut cards: Vec<Card> = hits
        .into_iter()
        .filter(|hit| query.matches_short_words(&hit.text))
        .filter(|hit| hit.media.is_none_or(|media| seen.insert(media)))
        .map(|hit| card(hit, rule))
        .filter(|card| {
            wanted.is_none_or(|kind| card.kind == kind || card.kind == MediaKind::Unknown)
        })
        .collect();
    sort_cards(&mut cards, Sort::TitleAscending);
    cards.truncate(MAX_RESULTS);
    Ok(cards)
}

fn card(hit: Hit, rule: WatchedRule) -> Card {
    let probed = hit.metadata.kind;
    let kind = if probed == MediaKind::Unknown { kind_by_extension(&hit.path) } else { probed };
    Card {
        label: display::title(&hit.metadata, Path::new(&hit.path)),
        media: hit.media,
        kind,
        watch: WatchState::of(&hit.saved, rule),
        path: hit.path,
        tags: hit.metadata,
        added_ms: hit.added_ms,
    }
}

#[cfg(test)]
mod tests;
