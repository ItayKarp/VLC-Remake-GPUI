//! A language's translations, looked up by their English text (and context).

use std::collections::HashMap;

use super::po::{self, Entry, ParseError};

/// Separates an entry's context from its English text in a key, as gettext does.
const CONTEXT_SEPARATOR: char = '\u{4}';

/// The translations of one language.
#[derive(Debug, Default)]
pub struct Catalog {
    /// Each translated entry's forms (one for plain text), by key.
    entries: HashMap<String, Vec<String>>,
}

impl Catalog {
    /// The translations in a `.po` file. Entries not translated yet are left out, so their
    /// English shows.
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "no real translation ships yet; the pseudo-locales use templates"
        )
    )]
    pub fn from_po(text: &str) -> Result<Self, ParseError> {
        let entries = po::parse(text)?
            .into_iter()
            .filter(|entry| entry.translations.iter().all(|form| !form.is_empty()))
            .filter(|entry| !entry.translations.is_empty())
            .map(|entry| (key(entry.context.as_deref(), &entry.id), entry.translations))
            .collect();
        Ok(Self { entries })
    }

    /// Made-up translations of every entry in a `.pot` template: `transform` applied to its
    /// English, and to its English plural.
    pub fn from_template(
        template: &str,
        transform: fn(&str) -> String,
    ) -> Result<Self, ParseError> {
        let entries = po::parse(template)?
            .into_iter()
            .map(|entry| (key(entry.context.as_deref(), &entry.id), forms(&entry, transform)))
            .collect();
        Ok(Self { entries })
    }

    /// The forms translating `text` in `context`, if it is translated.
    pub fn forms(&self, context: Option<&str>, text: &str) -> Option<&[String]> {
        let forms = match context {
            None => self.entries.get(text),
            Some(context) => self.entries.get(&key(Some(context), text)),
        };
        forms.map(Vec::as_slice)
    }

    /// How many entries are translated.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

fn forms(entry: &Entry, transform: fn(&str) -> String) -> Vec<String> {
    std::iter::once(&entry.id).chain(&entry.plural).map(|text| transform(text)).collect()
}

fn key(context: Option<&str>, text: &str) -> String {
    match context {
        None => text.to_owned(),
        Some(context) => format!("{context}{CONTEXT_SEPARATOR}{text}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GERMAN: &str = r#"
msgid "Play"
msgstr "Abspielen"

msgctxt "verb"
msgid "Open"
msgstr "Öffnen"

msgid "Untranslated"
msgstr ""

msgid "{count} item"
msgid_plural "{count} items"
msgstr[0] "{count} Element"
msgstr[1] "{count} Elemente"
"#;

    #[test]
    fn translations_are_found_by_english_and_context() {
        let catalog = Catalog::from_po(GERMAN).expect("parses");
        assert_eq!(catalog.forms(None, "Play"), Some(&["Abspielen".to_owned()][..]));
        assert_eq!(catalog.forms(Some("verb"), "Open").map(<[String]>::len), Some(1));
        assert_eq!(catalog.forms(None, "Open"), None);
        assert_eq!(catalog.forms(None, "Untranslated"), None);
        assert_eq!(catalog.forms(None, "{count} item").map(<[String]>::len), Some(2));
        assert_eq!(catalog.len(), 3);
    }

    #[test]
    fn a_template_gives_every_entry_made_up_forms() {
        let catalog = Catalog::from_template(GERMAN, str::to_uppercase).expect("parses");
        assert_eq!(catalog.forms(None, "Untranslated"), Some(&["UNTRANSLATED".to_owned()][..]));
        let plural = catalog.forms(None, "{count} item").expect("listed");
        assert_eq!(plural, ["{COUNT} ITEM", "{COUNT} ITEMS"]);
    }
}
