//! Numbers and dates as the interface language and the OS region write them (S9.2.1). Time
//! codes ("1:02:03") are not numbers here: they keep ASCII digits everywhere.

use chrono::{Datelike, Local, TimeZone};
use icu_datetime::DateTimeFormatter;
use icu_datetime::fieldsets::YMD;
use icu_datetime::input::Date;
use icu_decimal::DecimalFormatter;
use icu_decimal::input::Decimal;
use icu_decimal::options::DecimalFormatterOptions;
use icu_locale_core::Locale;

use super::active;

/// The formatters of one locale.
pub struct Formats {
    decimal: DecimalFormatter,
    date: DateTimeFormatter<YMD>,
}

impl Formats {
    /// The formatters for `locale`, or for English where ICU has no data for it.
    pub fn new(locale: &Locale) -> Self {
        Self::try_new(locale).unwrap_or_else(|| {
            tracing::warn!("i18n: no number or date formats for {locale}; using English");
            Self::try_new(&icu_locale_core::locale!("en")).expect("English formats are built in")
        })
    }

    fn try_new(locale: &Locale) -> Option<Self> {
        let decimal =
            DecimalFormatter::try_new(locale.into(), DecimalFormatterOptions::default()).ok()?;
        let date = DateTimeFormatter::try_new(locale.into(), YMD::medium()).ok()?;
        Some(Self { decimal, date })
    }

    /// `value` with the locale's digit grouping: 1,234 or 1.234.
    pub fn number(&self, value: impl Into<Decimal>) -> String {
        self.decimal.format(&value.into()).to_string()
    }

    /// `value` rounded to `fraction_digits` decimals, with the locale's separators: 1.4 or 1,4.
    pub fn decimal(&self, value: f64, fraction_digits: u8) -> String {
        let scale = 10_f64.powi(i32::from(fraction_digits));
        let mut decimal = Decimal::from((value * scale).round() as i64);
        let digits = i16::from(fraction_digits);
        decimal.absolute.multiply_pow10(-digits);
        decimal.absolute.pad_end(-digits);
        self.decimal.format(&decimal).to_string()
    }

    /// The local day of `ms` (milliseconds since the Unix epoch) as a medium date: "Nov 24,
    /// 2026" in American English, "24 Nov 2026" in British. `None` outside the calendar.
    pub fn date(&self, ms: i64) -> Option<String> {
        let local = Local.timestamp_millis_opt(ms).single()?;
        let month = u8::try_from(local.month()).ok()?;
        let day = u8::try_from(local.day()).ok()?;
        let date = Date::try_new_iso(local.year(), month, day).ok()?;
        Some(self.date.format(&date).to_string())
    }
}

/// `value` (any integer) as the interface writes whole numbers: "1,234".
pub fn number(value: impl Into<Decimal>) -> String {
    active::get().formats.number(value.into())
}

/// `value` with `fraction_digits` decimals, as the interface writes them.
pub fn decimal(value: f64, fraction_digits: u8) -> String {
    active::get().formats.decimal(value, fraction_digits)
}

/// The local date of `ms` (milliseconds since the Unix epoch), as the interface writes dates;
/// empty for a time no calendar has.
pub fn date(ms: i64) -> String {
    active::get().formats.date(ms).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn formats(tag: &str) -> Formats {
        Formats::new(&tag.parse().expect("a valid tag"))
    }

    #[test]
    fn numbers_are_grouped_as_the_locale_writes_them() {
        assert_eq!(formats("en").number(1_234_567), "1,234,567");
        assert_eq!(formats("de").number(1_234_567), "1.234.567");
        assert_eq!(formats("en").number(7), "7");
    }

    #[test]
    fn decimals_keep_their_digits_and_the_locale_separator() {
        assert_eq!(formats("en").decimal(1.44, 1), "1.4");
        assert_eq!(formats("en").decimal(2.0, 1), "2.0");
        assert_eq!(formats("de").decimal(1.45, 2), "1,45");
    }

    #[test]
    fn dates_follow_the_locale() {
        let noon = 1_795_521_600_000;
        assert_eq!(formats("en").date(noon).as_deref(), Some("Nov 24, 2026"));
        assert_eq!(formats("en-GB").date(noon).as_deref(), Some("24 Nov 2026"));
        assert_eq!(formats("en").date(i64::MAX), None);
    }
}
