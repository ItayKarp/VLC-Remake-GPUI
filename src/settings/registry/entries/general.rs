//! General ("Global Settings"): appearance and startup (S5.3.1–S5.3.3, S5.3.5). Diagnostics, on
//! the same page, are in `diagnostics.rs`.
//!
//! **Decision (D6):** the theme is offered Light, Dark, System, here and in onboarding, although
//! `global_01.png` draws Dark first.

use super::super::def::{Control, SettingDef};
use super::super::value::{Binding, Choice, SettingChoice, choice, toggle};
use crate::settings::InterfaceLanguage;
use crate::settings::registry::Category;
use crate::theme::{Accent, ThemeMode};

const APPEARANCE: (Category, &str) = (Category::General, tr_noop!("Appearance"));
const STARTUP: (Category, &str) = (Category::General, tr_noop!("Startup"));

/// The interface languages, and in debug builds the pseudo-locales after them (S9.2).
const LANGUAGES: &[InterfaceLanguage] = &[
    InterfaceLanguage::System,
    InterfaceLanguage::English,
    InterfaceLanguage::PseudoAccented,
    InterfaceLanguage::PseudoRtl,
];
const LANGUAGE_CHOICES: &[Choice] = &[
    Choice::new("system", tr_noop!("Same as the system")),
    Choice::new("english", "English"),
    Choice::new("pseudo-accented", "[Ƥšéûðö Åççéñţéð]"),
    Choice::new("pseudo-rtl", "Pseudo RTL"),
];
/// How many languages release builds list: the pseudo-locales are for checking the layout.
const RELEASED_LANGUAGES: usize = 2;
const LISTED_LANGUAGES: usize =
    if cfg!(debug_assertions) { LANGUAGES.len() } else { RELEASED_LANGUAGES };

impl SettingChoice for InterfaceLanguage {
    const VALUES: &'static [Self] = LANGUAGES.split_at(LISTED_LANGUAGES).0;
    const CHOICES: &'static [Choice] = LANGUAGE_CHOICES.split_at(LISTED_LANGUAGES).0;
}

impl SettingChoice for ThemeMode {
    const VALUES: &'static [Self] = &ThemeMode::ALL;
    const CHOICES: &'static [Choice] = &[
        Choice::new("light", tr_noop!("Light")),
        Choice::new("dark", tr_noop!("Dark")),
        Choice::new("system", tr_noop!("System")),
    ];
}

impl SettingChoice for Accent {
    const VALUES: &'static [Self] = &Accent::ALL;
    const CHOICES: &'static [Choice] = &[
        Choice::new("orange", tr_noop!("Orange")),
        Choice::new("red", tr_noop!("Red")),
        Choice::new("yellow", tr_noop!("Yellow")),
        Choice::new("green", tr_noop!("Green")),
        Choice::new("teal", tr_noop!("Teal")),
        Choice::new("blue", tr_noop!("Blue")),
        Choice::new("purple", tr_noop!("Purple")),
        Choice::new("pink", tr_noop!("Pink")),
        Choice::new("gray", tr_noop!("Gray")),
    ];
}

/// The accent swatch's color for a choice id.
fn accent_color(id: &str) -> Option<gpui::Hsla> {
    Accent::from_choice_id(id).map(Accent::color)
}

/// "Run Setup Again…": onboarding, starting from the current choices (S5.3.5). Deferred, since
/// it closes the Settings window this click came from.
fn run_setup_again(window: &mut gpui::Window, cx: &mut gpui::App) {
    let display = window.display(cx).map(|display| display.id());
    cx.defer(move |cx| crate::app::run_setup_again(display, cx));
}

/// The General page's appearance and startup rows.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    SettingDef::new(
        "language",
        APPEARANCE,
        tr_noop!("Interface language"),
        Control::Dropdown(InterfaceLanguage::CHOICES),
        choice!(prefs.language),
    )
    .info(tr_noop!("The language of menus, buttons and messages. More languages are on the way."))
    .keywords(&["locale", "translation", "english"]),
    SettingDef::new(
        "theme",
        APPEARANCE,
        tr_noop!("Theme"),
        Control::Segmented(ThemeMode::CHOICES),
        choice!(prefs.theme),
    )
    .info(tr_noop!("Dark or light windows, or whichever the system uses right now."))
    .keywords(&["appearance", "dark mode", "light mode", "color scheme"]),
    SettingDef::new(
        "accent",
        APPEARANCE,
        tr_noop!("Accent color"),
        Control::Swatches { choices: Accent::CHOICES, color: accent_color },
        choice!(prefs.accent),
    )
    .info(tr_noop!("The color of selected items, switches, sliders and the progress bar."))
    .keywords(&["highlight", "colour", "tint"]),
    SettingDef::new(
        "startup.show_dialogs",
        STARTUP,
        tr_noop!("Show confirmation and error dialogs"),
        Control::Toggle,
        toggle!(prefs.startup.show_dialogs),
    )
    .info(tr_noop!(
        "Asks before discarding or deleting something. Off, it happens at once; a deleted playlist \
         can still be brought back with Undo."
    ))
    .keywords(&["confirm", "ask", "prompt", "warning"]),
    SettingDef::new(
        "startup.check_for_updates",
        STARTUP,
        tr_noop!("Check for updates automatically"),
        Control::Toggle,
        toggle!(prefs.startup.check_for_updates),
    )
    .info(tr_noop!("Looks for a new version of VLC now and then, and offers to install it."))
    .keywords(&["update", "upgrade", "version", "new version"]),
    SettingDef::new(
        "run_setup_again",
        STARTUP,
        tr_noop!("Setup"),
        Control::Button { label: tr_noop!("Run Setup Again…"), action: run_setup_again },
        Binding::ACTION,
    )
    .info(tr_noop!(
        "Walks through the first-run setup again: media folder, theme and shortcuts, starting from \
         your current choices. Cancel leaves everything as it was."
    ))
    .keywords(&["onboarding", "welcome", "first run", "wizard"]),
];
