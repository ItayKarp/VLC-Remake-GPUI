//! The languages track labels can name: ISO 639-1 and both ISO 639-2 codes (bibliographic and
//! terminological), with the English name.

/// One language.
pub(super) struct Entry {
    /// ISO 639-1, or empty when the language has none.
    one: &'static str,
    /// ISO 639-2/B, used by Matroska and most containers.
    bibliographic: &'static str,
    /// ISO 639-2/T, used by MP4.
    terminological: &'static str,
    /// The English name.
    pub(super) name: &'static str,
}

impl Entry {
    /// The code a [`super::Language`] keeps: 639-1 when there is one.
    pub(super) fn canonical(&self) -> &'static str {
        if self.one.is_empty() { self.bibliographic } else { self.one }
    }
}

/// Shortens the table rows.
const fn e(
    one: &'static str,
    bibliographic: &'static str,
    terminological: &'static str,
    name: &'static str,
) -> Entry {
    Entry { one, bibliographic, terminological, name }
}

const LANGUAGES: &[Entry] = &[
    e("af", "afr", "afr", "Afrikaans"),
    e("am", "amh", "amh", "Amharic"),
    e("ar", "ara", "ara", "Arabic"),
    e("az", "aze", "aze", "Azerbaijani"),
    e("be", "bel", "bel", "Belarusian"),
    e("bg", "bul", "bul", "Bulgarian"),
    e("bn", "ben", "ben", "Bengali"),
    e("bs", "bos", "bos", "Bosnian"),
    e("ca", "cat", "cat", "Catalan"),
    e("cs", "cze", "ces", "Czech"),
    e("cy", "wel", "cym", "Welsh"),
    e("da", "dan", "dan", "Danish"),
    e("de", "ger", "deu", "German"),
    e("el", "gre", "ell", "Greek"),
    e("en", "eng", "eng", "English"),
    e("eo", "epo", "epo", "Esperanto"),
    e("es", "spa", "spa", "Spanish"),
    e("et", "est", "est", "Estonian"),
    e("eu", "baq", "eus", "Basque"),
    e("fa", "per", "fas", "Persian"),
    e("fi", "fin", "fin", "Finnish"),
    e("", "fil", "fil", "Filipino"),
    e("fr", "fre", "fra", "French"),
    e("ga", "gle", "gle", "Irish"),
    e("gl", "glg", "glg", "Galician"),
    e("gu", "guj", "guj", "Gujarati"),
    e("he", "heb", "heb", "Hebrew"),
    e("hi", "hin", "hin", "Hindi"),
    e("hr", "hrv", "hrv", "Croatian"),
    e("hu", "hun", "hun", "Hungarian"),
    e("hy", "arm", "hye", "Armenian"),
    e("id", "ind", "ind", "Indonesian"),
    e("is", "ice", "isl", "Icelandic"),
    e("it", "ita", "ita", "Italian"),
    e("ja", "jpn", "jpn", "Japanese"),
    e("ka", "geo", "kat", "Georgian"),
    e("kk", "kaz", "kaz", "Kazakh"),
    e("km", "khm", "khm", "Khmer"),
    e("kn", "kan", "kan", "Kannada"),
    e("ko", "kor", "kor", "Korean"),
    e("la", "lat", "lat", "Latin"),
    e("lo", "lao", "lao", "Lao"),
    e("lt", "lit", "lit", "Lithuanian"),
    e("lv", "lav", "lav", "Latvian"),
    e("mk", "mac", "mkd", "Macedonian"),
    e("ml", "mal", "mal", "Malayalam"),
    e("mn", "mon", "mon", "Mongolian"),
    e("mr", "mar", "mar", "Marathi"),
    e("ms", "may", "msa", "Malay"),
    e("my", "bur", "mya", "Burmese"),
    e("nb", "nob", "nob", "Norwegian Bokmål"),
    e("ne", "nep", "nep", "Nepali"),
    e("nl", "dut", "nld", "Dutch"),
    e("nn", "nno", "nno", "Norwegian Nynorsk"),
    e("no", "nor", "nor", "Norwegian"),
    e("pa", "pan", "pan", "Punjabi"),
    e("pl", "pol", "pol", "Polish"),
    e("pt", "por", "por", "Portuguese"),
    e("ro", "rum", "ron", "Romanian"),
    e("ru", "rus", "rus", "Russian"),
    e("si", "sin", "sin", "Sinhala"),
    e("sk", "slo", "slk", "Slovak"),
    e("sl", "slv", "slv", "Slovenian"),
    e("sq", "alb", "sqi", "Albanian"),
    e("sr", "srp", "srp", "Serbian"),
    e("sv", "swe", "swe", "Swedish"),
    e("sw", "swa", "swa", "Swahili"),
    e("ta", "tam", "tam", "Tamil"),
    e("te", "tel", "tel", "Telugu"),
    e("th", "tha", "tha", "Thai"),
    e("tl", "tgl", "tgl", "Tagalog"),
    e("tr", "tur", "tur", "Turkish"),
    e("uk", "ukr", "ukr", "Ukrainian"),
    e("ur", "urd", "urd", "Urdu"),
    e("vi", "vie", "vie", "Vietnamese"),
    e("yi", "yid", "yid", "Yiddish"),
    e("zh", "chi", "zho", "Chinese"),
];

/// Every language in the table, in table order.
pub(super) fn all() -> impl Iterator<Item = &'static Entry> {
    LANGUAGES.iter()
}

/// The language with the lower-case code `code`, in any of its three forms.
pub(super) fn find(code: &str) -> Option<&'static Entry> {
    LANGUAGES.iter().find(|entry| {
        (!entry.one.is_empty() && entry.one == code)
            || entry.bibliographic == code
            || entry.terminological == code
    })
}

/// The language whose English name is `name`, ignoring case (some containers write names).
pub(super) fn by_name(name: &str) -> Option<&'static Entry> {
    LANGUAGES.iter().find(|entry| entry.name.eq_ignore_ascii_case(name))
}
