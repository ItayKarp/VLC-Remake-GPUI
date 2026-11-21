//! Track languages: the codes containers tag tracks with (`en`, `eng`, `fre`, `pt-BR`), made
//! comparable, and their English names for track labels.

mod table;

/// Codes that tag a track without naming a language: undetermined, no linguistic content,
/// uncoded, multiple.
const UNTAGGED: [&str; 4] = ["und", "zxx", "mis", "mul"];

/// A track's language, normalized so `en`, `eng` and `en-US` compare equal.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Language {
    /// The ISO 639-1 code when the language has one, else the 639-2 code; for text that is no
    /// known code, the text as the container wrote it.
    code: String,
}

impl Language {
    /// Reads a container's language tag or a language setting. `None` for an empty tag and for
    /// "undetermined" codes (`und`), which label the track "Track N" instead.
    pub fn parse(tag: &str) -> Option<Self> {
        let tag = tag.trim();
        let primary = tag.split(['-', '_']).next().unwrap_or_default().to_ascii_lowercase();
        if primary.is_empty() || UNTAGGED.contains(&primary.as_str()) {
            return None;
        }
        let code = match table::find(&primary).or_else(|| table::by_name(tag)) {
            Some(entry) => entry.canonical().to_owned(),
            None if primary.len() <= 3 => primary,
            None => tag.to_owned(),
        };
        Some(Self { code })
    }

    /// The normalized code (`en`, or `tlh` for a language without a 639-1 code).
    pub fn code(&self) -> &str {
        &self.code
    }

    /// The language's English name ("English"); an unknown code shows in capitals ("TLH"), and
    /// unknown text as written.
    pub fn name(&self) -> String {
        match table::find(&self.code) {
            Some(entry) => entry.name.to_owned(),
            None if self.code.len() <= 3 => self.code.to_ascii_uppercase(),
            None => self.code.clone(),
        }
    }
}

/// Reads a list of language settings, skipping blanks and duplicates, in order.
pub fn parse_list(tags: &[String]) -> Vec<Language> {
    let mut languages: Vec<Language> = Vec::new();
    for language in tags.iter().filter_map(|tag| Language::parse(tag)) {
        if !languages.contains(&language) {
            languages.push(language);
        }
    }
    languages
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(tag: &str) -> Option<String> {
        Language::parse(tag).map(|language| language.code().to_owned())
    }

    #[test]
    fn two_and_three_letter_codes_compare_equal() {
        assert_eq!(Language::parse("eng"), Language::parse("en"));
        assert_eq!(Language::parse("fre"), Language::parse("fra"));
        assert_eq!(code("pt-BR").as_deref(), Some("pt"));
        assert_eq!(code(" GER ").as_deref(), Some("de"));
        assert_eq!(code("English").as_deref(), Some("en"));
    }

    #[test]
    fn undetermined_tags_have_no_language() {
        for tag in ["", "  ", "und", "UND", "zxx", "mul"] {
            assert_eq!(Language::parse(tag), None, "{tag:?}");
        }
    }

    #[test]
    fn names_are_english_and_unknown_codes_stay_visible() {
        assert_eq!(Language::parse("jpn").unwrap().name(), "Japanese");
        assert_eq!(Language::parse("he").unwrap().name(), "Hebrew");
        assert_eq!(Language::parse("tlh").unwrap().name(), "TLH");
        assert_eq!(Language::parse("Klingon").unwrap().name(), "Klingon");
    }

    #[test]
    fn lists_drop_blanks_and_repeats() {
        let tags = ["en", "", "eng", "fr"].map(String::from);
        let codes: Vec<_> = parse_list(&tags).iter().map(|l| l.code().to_owned()).collect();
        assert_eq!(codes, ["en", "fr"]);
    }
}
