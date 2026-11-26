//! The languages the interface can be drawn in, and which one Settings → General → Interface
//! language and the OS ask for (S9.2.4).

use icu_locale_core::Locale;

use super::direction::Direction;
use crate::settings::InterfaceLanguage;

/// A language the interface has text for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    /// English, the language the code is written in.
    English,
    /// English made about 40% longer and accented ("[Ƥļåŷ·····]"), to find text that doesn't
    /// fit (S9.2.3) and text that isn't marked for translation (it stays plain).
    PseudoAccented,
    /// English in right-to-left paragraphs, to try the mirrored layout (S9.2.2).
    PseudoRtl,
}

impl Language {
    /// The language the interface uses for the user's choice: the OS's first preferred
    /// language the interface has, English otherwise.
    pub fn resolve(choice: InterfaceLanguage) -> Self {
        match choice {
            InterfaceLanguage::System => Self::from_os(&os_locales()),
            InterfaceLanguage::English => Self::English,
            InterfaceLanguage::PseudoAccented => Self::PseudoAccented,
            InterfaceLanguage::PseudoRtl => Self::PseudoRtl,
        }
    }

    /// The first of the OS's preferred languages the interface has, English otherwise. Only
    /// English is translated so far, so every OS language reads English.
    fn from_os(preferred: &[Locale]) -> Self {
        preferred
            .iter()
            .find_map(|locale| Self::with_code(locale.id.language.as_str()))
            .unwrap_or(Self::English)
    }

    /// The real language whose ISO 639 code is `code`.
    fn with_code(code: &str) -> Option<Self> {
        (code == "en").then_some(Self::English)
    }

    /// The BCP 47 tag of the language. The pseudo-locales use the tags Android and Chrome give
    /// theirs.
    pub fn tag(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::PseudoAccented => "en-XA",
            Self::PseudoRtl => "ar-XB",
        }
    }

    /// Which way the language is written, and so which way the layout runs.
    pub fn direction(self) -> Direction {
        match self {
            Self::English | Self::PseudoAccented => Direction::Ltr,
            Self::PseudoRtl => Direction::Rtl,
        }
    }

    /// The locale numbers and dates are written in: the OS's region when the OS prefers this
    /// language ("en-GB" writes 24/11/2026), the language's own otherwise. The pseudo-locales
    /// are English underneath.
    pub fn format_locale(self) -> Locale {
        format_locale_among(self, &os_locales())
    }
}

/// [`Language::format_locale`], given the OS's preferred locales.
fn format_locale_among(language: Language, preferred: &[Locale]) -> Locale {
    let english = icu_locale_core::locale!("en");
    let base = match language {
        Language::English | Language::PseudoAccented | Language::PseudoRtl => english,
    };
    preferred.iter().find(|locale| locale.id.language == base.id.language).cloned().unwrap_or(base)
}

/// The OS's preferred languages, most preferred first. Tags that don't parse (POSIX's "C") are
/// skipped.
fn os_locales() -> Vec<Locale> {
    sys_locale::get_locales()
        .filter_map(|tag| tag.replace('_', "-").split('.').next()?.parse::<Locale>().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn locales(tags: &[&str]) -> Vec<Locale> {
        tags.iter().map(|tag| tag.parse().expect("a valid tag")).collect()
    }

    #[test]
    fn the_os_language_is_used_when_the_interface_has_it_and_english_otherwise() {
        assert_eq!(Language::from_os(&locales(&["he-IL", "en-US"])), Language::English);
        assert_eq!(Language::from_os(&locales(&["de-DE"])), Language::English);
        assert_eq!(Language::from_os(&[]), Language::English);
    }

    #[test]
    fn numbers_and_dates_follow_the_os_region_of_the_same_language() {
        let preferred = locales(&["de-DE", "en-GB"]);
        assert_eq!(format_locale_among(Language::English, &preferred).to_string(), "en-GB");
        assert_eq!(format_locale_among(Language::English, &locales(&["fr"])).to_string(), "en");
    }

    #[test]
    fn only_the_rtl_pseudo_locale_mirrors() {
        assert_eq!(Language::English.direction(), Direction::Ltr);
        assert_eq!(Language::PseudoAccented.direction(), Direction::Ltr);
        assert_eq!(Language::PseudoRtl.direction(), Direction::Rtl);
    }
}
