//! The settings registry (S5.1): one declarative list of every setting the Settings window shows.
//!
//! Each [`SettingDef`] names its page and section, label and `(i)` text, control, where its value
//! lives ([`Binding`]), the range and checks a value must pass, the setting it depends on, when a
//! change takes effect ([`ApplyMode`]), whether it is advanced, and extra search words. From that
//! one list the Settings window draws its pages (`settings_view`), search finds rows, typed values
//! are validated, values from other versions are clamped on load ([`sanitize`]), and the restart
//! banner knows when to show ([`needs_restart`]).
//!
//! A story that adds a setting adds its row here, in its category's file under `entries/`.

mod category;
mod def;
mod entries;
mod rules;
#[cfg(test)]
mod tests;
pub(crate) mod value;

#[allow(
    unused_imports,
    reason = "the Settings window (S5.2) picks hand-built pages and searches"
)]
pub use category::Layout;
pub use category::{Category, Group};
pub use def::{ApplyMode, Control, SettingDef};
#[cfg(test)]
pub use rules::search;
pub use rules::{hex_color, sanitize, search_words};
pub use value::{Binding, Choice, SettingChoice, Value};

use super::Settings;

/// Every setting, in sidebar order and then page order.
pub fn all() -> impl Iterator<Item = &'static SettingDef> {
    entries::PAGES.iter().flat_map(|page| page.iter())
}

/// The setting with `key`.
pub fn find(key: &str) -> Option<&'static SettingDef> {
    all().find(|def| def.key == key)
}

/// The settings on `category`'s page, in order.
pub fn in_category(category: Category) -> impl Iterator<Item = &'static SettingDef> {
    all().filter(move |def| def.category == category)
}

/// Whether a setting that applies at the next launch differs from what this launch started with.
pub fn needs_restart(at_launch: &Settings, now: &Settings) -> bool {
    all()
        .filter(|def| def.apply == ApplyMode::Restart)
        .any(|def| def.value(at_launch) != def.value(now))
}
