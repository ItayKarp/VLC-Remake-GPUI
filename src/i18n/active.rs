//! The interface language in use, readable from anywhere without an `App`, and the lookups
//! that translate through it.
//!
//! Each language applied is built once and kept for the rest of the run (a switch keeps the
//! old one too), so translations can be handed out as `&'static str`. Switching is rare: one
//! per change in Settings.

use std::sync::{OnceLock, RwLock};

use icu_plurals::{PluralCategory, PluralRules};

use super::catalog::Catalog;
use super::formats::Formats;
use super::language::Language;
use super::pseudo;

/// The `.pot` template, which the pseudo-locales translate every entry of.
const TEMPLATE: &str = include_str!("../../assets/i18n/messages.pot");

/// A language, ready to translate with.
pub struct Active {
    /// The language.
    pub language: Language,
    /// Its translations; `None` for English, whose text is the code's.
    catalog: Option<Catalog>,
    /// Its plural rules, which pick a translation's form.
    plural_rules: Option<PluralRules>,
    /// How it writes numbers and dates.
    pub formats: Formats,
}

impl Active {
    fn new(language: Language) -> Self {
        let catalog = match language {
            Language::English => None,
            Language::PseudoAccented => template_catalog(pseudo::accented),
            Language::PseudoRtl => template_catalog(pseudo::right_to_left),
        };
        let plural_rules = match language {
            Language::English | Language::PseudoAccented | Language::PseudoRtl => None,
        };
        Self { language, catalog, plural_rules, formats: Formats::new(&language.format_locale()) }
    }

    fn translated(&self, context: Option<&str>, text: &str) -> Option<&str> {
        self.catalog.as_ref()?.forms(context, text)?.first().map(String::as_str)
    }

    /// The index of `count`'s form among a translation's plural forms: the position of its
    /// CLDR category among the language's categories. English text (no rules) has one and
    /// other.
    fn form_index(&self, count: u64) -> usize {
        let Some(rules) = &self.plural_rules else { return usize::from(count != 1) };
        let category = rules.category_for(count);
        rules.categories().position(|each| each == category).unwrap_or_else(|| {
            rules.categories().position(|each| each == PluralCategory::Other).unwrap_or_default()
        })
    }
}

fn template_catalog(transform: fn(&str) -> String) -> Option<Catalog> {
    Catalog::from_template(TEMPLATE, transform)
        .inspect_err(|error| tracing::error!("i18n: the message template is broken: {error}"))
        .ok()
}

/// The language in use; English until [`set`] is called.
static ACTIVE: RwLock<Option<&'static Active>> = RwLock::new(None);

/// The language in use.
pub fn get() -> &'static Active {
    let current = *ACTIVE.read().unwrap_or_else(std::sync::PoisonError::into_inner);
    current.unwrap_or_else(english)
}

fn english() -> &'static Active {
    static ENGLISH: OnceLock<Active> = OnceLock::new();
    ENGLISH.get_or_init(|| Active::new(Language::English))
}

/// Translates from now on into `language`.
pub fn set(language: Language) {
    let active: &'static Active = if language == Language::English {
        english()
    } else {
        Box::leak(Box::new(Active::new(language)))
    };
    *ACTIVE.write().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(active);
}

/// `text` in the interface language; `text` itself where it isn't translated.
pub fn text(text: &'static str) -> &'static str {
    get().translated(None, text).unwrap_or(text)
}

/// `text`, meaning what `context` says, in the interface language.
pub fn text_in(context: &'static str, text: &'static str) -> &'static str {
    get().translated(Some(context), text).unwrap_or(text)
}

/// The form of `singular`/`plural` for `count`, in the interface language.
pub fn plural(singular: &'static str, plural: &'static str, count: u64) -> &'static str {
    let active = get();
    let index = active.form_index(count);
    let forms = active.catalog.as_ref().and_then(|catalog| catalog.forms(None, singular));
    match forms {
        Some(forms) => forms.get(index).or(forms.last()).map_or(singular, String::as_str),
        None if count == 1 => singular,
        None => plural,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_is_the_code_s_own_text() {
        let english = Active::new(Language::English);
        assert_eq!(english.translated(None, "Play"), None);
        assert_eq!(english.form_index(1), 0);
        assert_eq!(english.form_index(0), 1);
        assert_eq!(english.form_index(2), 1);
    }

    #[test]
    fn the_pseudo_locales_translate_every_template_entry() {
        let accented = Active::new(Language::PseudoAccented);
        let catalog = accented.catalog.as_ref().expect("the template parses");
        assert_eq!(catalog.len(), super::super::po::parse(TEMPLATE).expect("parses").len());
    }

    #[test]
    fn plural_rules_pick_a_form_by_category() {
        let russian = PluralRules::try_new_cardinal(icu_locale_core::locale!("ru").into())
            .expect("Russian rules are built in");
        let active = Active { plural_rules: Some(russian), ..Active::new(Language::English) };
        assert_eq!([1, 3, 5, 21].map(|count| active.form_index(count)), [0, 1, 2, 0]);
    }
}
