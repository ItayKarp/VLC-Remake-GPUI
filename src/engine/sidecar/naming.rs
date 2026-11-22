//! What a subtitle file's name says besides the video's title (S6.1.3): its language
//! (`Movie.en.srt`, `Movie_English.srt`, `Movie.pt-BR.srt`, `Subs/Movie/2_English.srt`) and
//! whether it is forced (`Movie.en.forced.srt`).

use crate::engine::language::Language;

/// Words that look like a language code but mean something else in subtitle names: `hi` is
/// "hearing impaired" far more often than Hindi.
const NOT_LANGUAGES: [&str; 1] = ["hi"];
/// The word that marks forced subtitles.
const FORCED: &str = "forced";

/// What a subtitle file's name tells about it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Naming {
    /// The language the name gives, if any.
    pub(super) language: Option<Language>,
    /// The name marks it forced: only the lines in another language than the audio.
    pub(super) forced: bool,
}

/// One word of a name, and whether a hyphen joins it to the word before (`BR` in `pt-BR`).
struct Word<'a> {
    text: &'a str,
    hyphenated: bool,
}

/// Reads `name` (a file name without its extension), ignoring the words of the video's `title`.
/// The language is the last word naming one, since it usually follows the title and release
/// details; a region or script after a hyphen (`pt-BR`, `zh-Hans`) belongs to the language
/// before it.
pub(super) fn read(name: &str, title: &str) -> Naming {
    let title_words: Vec<String> = words(title).map(|word| word.text.to_lowercase()).collect();
    let words: Vec<Word> =
        words(name).filter(|word| !title_words.contains(&word.text.to_lowercase())).collect();
    let forced = words.iter().any(|word| word.text.eq_ignore_ascii_case(FORCED));
    let language = (0..words.len()).rev().find_map(|index| {
        let word = &words[index];
        let qualifies = index > 0 && word.hyphenated && language(words[index - 1].text).is_some();
        if qualifies { None } else { language(word.text) }
    });
    Naming { language, forced }
}

/// The language `word` names, unless it is one of [`NOT_LANGUAGES`].
fn language(word: &str) -> Option<Language> {
    if NOT_LANGUAGES.iter().any(|other| word.eq_ignore_ascii_case(other)) {
        return None;
    }
    Language::recognize(word)
}

/// The letters-and-digits runs of `text`, each knowing whether a hyphen came right before it.
fn words(text: &str) -> impl Iterator<Item = Word<'_>> {
    let mut words = Vec::new();
    let mut start = None;
    // A space after the last character ends the last word.
    for (index, character) in text.char_indices().chain([(text.len(), ' ')]) {
        match (character.is_alphanumeric(), start) {
            (true, None) => start = Some(index),
            (false, Some(from)) => {
                let hyphenated = text[..from].ends_with('-');
                words.push(Word { text: &text[from..index], hyphenated });
                start = None;
            }
            _ => {}
        }
    }
    words.into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(name: &str, title: &str) -> Option<String> {
        read(name, title).language.map(|language| language.code().to_owned())
    }

    #[test]
    fn codes_and_names_after_the_title_give_the_language() {
        assert_eq!(code("Movie.en", "Movie").as_deref(), Some("en"));
        assert_eq!(code("Movie_English", "Movie").as_deref(), Some("en"));
        assert_eq!(code("Movie.HEB", "Movie").as_deref(), Some("he"));
        assert_eq!(code("Movie.2020.1080p.fre.forced", "Movie").as_deref(), Some("fr"));
        assert_eq!(code("2_English", "Movie").as_deref(), Some("en"));
    }

    #[test]
    fn a_region_belongs_to_the_language_before_it() {
        assert_eq!(code("Movie.pt-BR", "Movie").as_deref(), Some("pt"));
        assert_eq!(code("Movie-en", "Movie").as_deref(), Some("en"));
    }

    #[test]
    fn the_title_and_other_words_name_no_language() {
        assert_eq!(code("French Kiss", "French Kiss"), None);
        assert_eq!(code("French Kiss.de", "French Kiss").as_deref(), Some("de"));
        assert_eq!(code("Movie.HI", "Movie"), None);
        assert_eq!(code("Movie.sdh", "Movie"), None);
        assert_eq!(code("Movie", "Movie"), None);
    }

    #[test]
    fn forced_is_read_from_the_name() {
        assert!(read("Movie.en.FORCED", "Movie").forced);
        assert!(!read("Movie.en", "Movie").forced);
    }
}
