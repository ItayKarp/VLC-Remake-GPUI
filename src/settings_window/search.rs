//! What a search in the Settings window finds (S5.2.3): how many rows match on each page, which
//! pages the sidebar offers while it searches, and where Up and Down go among them.

use crate::settings::registry::{self, Category};

/// A search as typed, and the matches on each page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Search {
    /// The query as typed, for the empty state.
    query: String,
    /// The query's folded words; empty when there is no search.
    words: Vec<String>,
    /// The matching rows per page, in [`Category::ALL`] order.
    counts: [usize; Category::ALL.len()],
}

impl Search {
    /// The search for `query`.
    pub(super) fn new(query: &str) -> Self {
        let words = registry::search_words(query);
        let mut counts = [0; Category::ALL.len()];
        if !words.is_empty() {
            for def in registry::all().filter(|def| def.matches(&words)) {
                counts[index(def.category)] += 1;
            }
        }
        Self { query: query.trim().to_owned(), words, counts }
    }

    /// The query as typed, trimmed.
    pub(super) fn query(&self) -> &str {
        &self.query
    }

    /// The folded words pages filter by.
    pub(super) fn words(&self) -> &[String] {
        &self.words
    }

    /// Whether anything is being searched for.
    pub(super) fn is_active(&self) -> bool {
        !self.words.is_empty()
    }

    /// How many rows on `category`'s page match.
    pub(super) fn count(&self, category: Category) -> usize {
        self.counts[index(category)]
    }

    /// Whether a search is on and nothing matches anywhere.
    pub(super) fn finds_nothing(&self) -> bool {
        self.is_active() && self.counts.iter().all(|&count| count == 0)
    }

    /// Whether the sidebar offers `category`: always without a search, and with one only when
    /// its page has a match.
    pub(super) fn offers(&self, category: Category) -> bool {
        !self.is_active() || self.count(category) > 0
    }

    /// The first category the sidebar offers.
    pub(super) fn first_offered(&self) -> Option<Category> {
        Category::ALL.into_iter().find(|&category| self.offers(category))
    }

    /// The offered category before (`forward` false) or after `current`, if there is one.
    pub(super) fn step(&self, current: Category, forward: bool) -> Option<Category> {
        let at = index(current);
        let offered = |category: &&Category| self.offers(**category);
        if forward {
            Category::ALL[at + 1..].iter().find(offered).copied()
        } else {
            Category::ALL[..at].iter().rev().find(offered).copied()
        }
    }
}

/// `category`'s place in the sidebar.
fn index(category: Category) -> usize {
    Category::ALL.iter().position(|&c| c == category).expect("every category is in Category::ALL")
}

#[cfg(test)]
mod tests {
    use super::Search;
    use crate::settings::registry::Category;

    #[test]
    fn no_search_offers_every_category_in_order() {
        let search = Search::new("  ");
        assert!(!search.is_active());
        assert!(Category::ALL.iter().all(|&category| search.offers(category)));
        assert_eq!(search.step(Category::General, true), Some(Category::Interface));
        assert_eq!(search.step(Category::General, false), None);
        let last = *Category::ALL.last().expect("categories");
        assert_eq!(search.step(last, true), None);
    }

    #[test]
    fn a_search_offers_only_pages_with_matches() {
        let search = Search::new("subtitles");
        assert!(search.count(Category::Subtitles) > 0);
        assert!(!search.offers(Category::Network));
        let first = search.first_offered().expect("a page matches");
        assert!(search.offers(first));
        if let Some(next) = search.step(first, true) {
            assert!(search.count(next) > 0);
        }
    }

    #[test]
    fn a_search_without_matches_finds_nothing() {
        let search = Search::new("zzzqqq");
        assert!(search.finds_nothing());
        assert_eq!(search.first_offered(), None);
        assert_eq!(search.query(), "zzzqqq");
    }
}
