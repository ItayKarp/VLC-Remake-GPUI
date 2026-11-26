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
    e("af", "afr", "afr", tr_noop!("Afrikaans")),
    e("am", "amh", "amh", tr_noop!("Amharic")),
    e("ar", "ara", "ara", tr_noop!("Arabic")),
    e("az", "aze", "aze", tr_noop!("Azerbaijani")),
    e("be", "bel", "bel", tr_noop!("Belarusian")),
    e("bg", "bul", "bul", tr_noop!("Bulgarian")),
    e("bn", "ben", "ben", tr_noop!("Bengali")),
    e("bs", "bos", "bos", tr_noop!("Bosnian")),
    e("ca", "cat", "cat", tr_noop!("Catalan")),
    e("cs", "cze", "ces", tr_noop!("Czech")),
    e("cy", "wel", "cym", tr_noop!("Welsh")),
    e("da", "dan", "dan", tr_noop!("Danish")),
    e("de", "ger", "deu", tr_noop!("German")),
    e("el", "gre", "ell", tr_noop!("Greek")),
    e("en", "eng", "eng", tr_noop!("English")),
    e("eo", "epo", "epo", tr_noop!("Esperanto")),
    e("es", "spa", "spa", tr_noop!("Spanish")),
    e("et", "est", "est", tr_noop!("Estonian")),
    e("eu", "baq", "eus", tr_noop!("Basque")),
    e("fa", "per", "fas", tr_noop!("Persian")),
    e("fi", "fin", "fin", tr_noop!("Finnish")),
    e("", "fil", "fil", tr_noop!("Filipino")),
    e("fr", "fre", "fra", tr_noop!("French")),
    e("ga", "gle", "gle", tr_noop!("Irish")),
    e("gl", "glg", "glg", tr_noop!("Galician")),
    e("gu", "guj", "guj", tr_noop!("Gujarati")),
    e("he", "heb", "heb", tr_noop!("Hebrew")),
    e("hi", "hin", "hin", tr_noop!("Hindi")),
    e("hr", "hrv", "hrv", tr_noop!("Croatian")),
    e("hu", "hun", "hun", tr_noop!("Hungarian")),
    e("hy", "arm", "hye", tr_noop!("Armenian")),
    e("id", "ind", "ind", tr_noop!("Indonesian")),
    e("is", "ice", "isl", tr_noop!("Icelandic")),
    e("it", "ita", "ita", tr_noop!("Italian")),
    e("ja", "jpn", "jpn", tr_noop!("Japanese")),
    e("ka", "geo", "kat", tr_noop!("Georgian")),
    e("kk", "kaz", "kaz", tr_noop!("Kazakh")),
    e("km", "khm", "khm", tr_noop!("Khmer")),
    e("kn", "kan", "kan", tr_noop!("Kannada")),
    e("ko", "kor", "kor", tr_noop!("Korean")),
    e("la", "lat", "lat", tr_noop!("Latin")),
    e("lo", "lao", "lao", tr_noop!("Lao")),
    e("lt", "lit", "lit", tr_noop!("Lithuanian")),
    e("lv", "lav", "lav", tr_noop!("Latvian")),
    e("mk", "mac", "mkd", tr_noop!("Macedonian")),
    e("ml", "mal", "mal", tr_noop!("Malayalam")),
    e("mn", "mon", "mon", tr_noop!("Mongolian")),
    e("mr", "mar", "mar", tr_noop!("Marathi")),
    e("ms", "may", "msa", tr_noop!("Malay")),
    e("my", "bur", "mya", tr_noop!("Burmese")),
    e("nb", "nob", "nob", tr_noop!("Norwegian Bokmål")),
    e("ne", "nep", "nep", tr_noop!("Nepali")),
    e("nl", "dut", "nld", tr_noop!("Dutch")),
    e("nn", "nno", "nno", tr_noop!("Norwegian Nynorsk")),
    e("no", "nor", "nor", tr_noop!("Norwegian")),
    e("pa", "pan", "pan", tr_noop!("Punjabi")),
    e("pl", "pol", "pol", tr_noop!("Polish")),
    e("pt", "por", "por", tr_noop!("Portuguese")),
    e("ro", "rum", "ron", tr_noop!("Romanian")),
    e("ru", "rus", "rus", tr_noop!("Russian")),
    e("si", "sin", "sin", tr_noop!("Sinhala")),
    e("sk", "slo", "slk", tr_noop!("Slovak")),
    e("sl", "slv", "slv", tr_noop!("Slovenian")),
    e("sq", "alb", "sqi", tr_noop!("Albanian")),
    e("sr", "srp", "srp", tr_noop!("Serbian")),
    e("sv", "swe", "swe", tr_noop!("Swedish")),
    e("sw", "swa", "swa", tr_noop!("Swahili")),
    e("ta", "tam", "tam", tr_noop!("Tamil")),
    e("te", "tel", "tel", tr_noop!("Telugu")),
    e("th", "tha", "tha", tr_noop!("Thai")),
    e("tl", "tgl", "tgl", tr_noop!("Tagalog")),
    e("tr", "tur", "tur", tr_noop!("Turkish")),
    e("uk", "ukr", "ukr", tr_noop!("Ukrainian")),
    e("ur", "urd", "urd", tr_noop!("Urdu")),
    e("vi", "vie", "vie", tr_noop!("Vietnamese")),
    e("yi", "yid", "yid", tr_noop!("Yiddish")),
    e("zh", "chi", "zho", tr_noop!("Chinese")),
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
