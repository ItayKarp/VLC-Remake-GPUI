//! Internationalization (S9.2): every string the interface shows goes through here, in the
//! interface language, with that language's plural rules, number and date formats and layout
//! direction.
//!
//! **Decision:** gettext's model. The code keeps the English text, marked with a macro, and the
//! English is the key a translation is looked up by:
//!
//! - [`tr!`]`("Play")` is the text in the interface language (`&'static str`);
//!   `tr!("Playing {title}", title)` or `tr!("…{name}…", name = value)` fills placeholders and
//!   gives a `String`.
//! - [`trn!`]`("{count} item", "{count} items", n)` picks the plural form for `n` and fills
//!   `{count}` with it, grouped as the language writes numbers.
//! - [`trc!`]`("context", "Open")` is for one English word with two meanings.
//! - [`tr_noop!`]`("Light")` only marks text in a `const` or `static` table, where nothing can
//!   be looked up; whoever draws it translates it with [`text`].
//!
//! A test (`extract`) collects every marked string into `assets/i18n/messages.pot`, the
//! template translators work from, and fails while that file is stale. Translations are
//! gettext `.po` files; plural forms are listed in CLDR's order of the language's categories
//! (zero, one, two, few, many, other), whatever a `.po`'s `Plural-Forms` header says.
//!
//! The language is a process-wide value, like the text scale, so text can be translated
//! anywhere without an `App`. [`init`] sets it from Settings → General → Interface language and
//! follows changes live: every window draws again and the macOS menu bar is rebuilt. Text a
//! view keeps (a toast already shown) stays in the language it was made in. Tests never change
//! it, so they read English.

mod active;
mod catalog;
mod direction;
#[cfg(test)]
mod extract;
mod fill;
mod formats;
mod language;
mod layout;
mod po;
mod pseudo;

use gpui::{App, Context, Global, Subscription, Window};

pub use active::{plural, text, text_in};
pub use fill::{Count, fill};
pub use formats::{date, decimal, number};
pub use language::Language;
pub use layout::{DirectionalStyle, mirrored};

use crate::settings::{self, SettingsStore};

/// The language [`init`] last applied, so saving unrelated settings changes nothing.
struct Applied(Language);

impl Global for Applied {}

/// Applies the interface language from settings and follows its changes. Call after
/// [`settings::init`] and before any window opens.
pub fn init(cx: &mut App) {
    let language = Language::resolve(settings::get(cx).prefs.language);
    active::set(language);
    cx.set_global(Applied(language));
    cx.observe_global::<SettingsStore>(follow_settings).detach();
}

/// Calls `rebuild` after the interface language changes, for a view that keeps text it made
/// once (a dropdown's options, a field's placeholder). Views that only draw text need nothing:
/// every window draws again.
pub fn on_change<V: 'static>(
    window: &mut Window,
    cx: &mut Context<V>,
    rebuild: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> Subscription {
    cx.observe_global_in::<Applied>(window, rebuild)
}

fn follow_settings(cx: &mut App) {
    let language = Language::resolve(settings::get(cx).prefs.language);
    if cx.global::<Applied>().0 == language {
        return;
    }
    tracing::info!("i18n: interface language is now {}", language.tag());
    active::set(language);
    cx.set_global(Applied(language));
    crate::keymap::refresh_menus(cx);
    cx.refresh_windows();
}

/// The interface text for `text`, in the interface language: `tr!("Play")`, or with
/// placeholders filled, `tr!("Playing {title}", title)` and `tr!("{a} of {b}", a = x, b = y)`.
macro_rules! tr {
    ($text:literal) => {
        $crate::i18n::text($text)
    };
    ($text:literal, $($name:ident $(= $value:expr)?),+ $(,)?) => {
        $crate::i18n::fill($crate::i18n::text($text), &[$(tr!(@arg $name $(= $value)?)),+])
    };
    (@arg $name:ident) => {
        (stringify!($name), &$name as &dyn ::std::fmt::Display)
    };
    (@arg $name:ident = $value:expr) => {
        (stringify!($name), &$value as &dyn ::std::fmt::Display)
    };
}

/// The plural form of `singular`/`plural` for `count`, with `{count}` filled in as the
/// language writes numbers, and any other placeholders as in [`tr!`].
macro_rules! trn {
    ($singular:literal, $plural:literal, $count:expr) => {
        trn!($singular, $plural, $count,)
    };
    ($singular:literal, $plural:literal, $count:expr, $($name:ident $(= $value:expr)?),* $(,)?) => {{
        let count = $crate::i18n::Count::into_count($count);
        let number = $crate::i18n::number(count);
        $crate::i18n::fill(
            $crate::i18n::plural($singular, $plural, count),
            &[("count", &number as &dyn ::std::fmt::Display) $(, tr!(@arg $name $(= $value)?))*],
        )
    }};
}

/// Like [`tr!`], for English text that means two things: `context` tells them apart for
/// translators ("verb", "media menu").
macro_rules! trc {
    ($context:literal, $text:literal) => {
        $crate::i18n::text_in($context, $text)
    };
    ($context:literal, $text:literal, $($name:ident $(= $value:expr)?),+ $(,)?) => {
        $crate::i18n::fill(
            $crate::i18n::text_in($context, $text),
            &[$(tr!(@arg $name $(= $value)?)),+],
        )
    };
}

/// Marks `text` for translation without translating it, for `const` and `static` tables; it
/// is the literal itself. Translate it where it is drawn with [`text`].
macro_rules! tr_noop {
    ($text:literal) => {
        $text
    };
}
