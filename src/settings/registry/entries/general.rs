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

const APPEARANCE: (Category, &str) = (Category::General, "Appearance");
const STARTUP: (Category, &str) = (Category::General, "Startup");

impl SettingChoice for InterfaceLanguage {
    const VALUES: &'static [Self] = &[Self::System, Self::English];
    const CHOICES: &'static [Choice] =
        &[Choice::new("system", "Same as the system"), Choice::new("english", "English")];
}

impl SettingChoice for ThemeMode {
    const VALUES: &'static [Self] = &ThemeMode::ALL;
    const CHOICES: &'static [Choice] = &[
        Choice::new("light", "Light"),
        Choice::new("dark", "Dark"),
        Choice::new("system", "System"),
    ];
}

impl SettingChoice for Accent {
    const VALUES: &'static [Self] = &Accent::ALL;
    const CHOICES: &'static [Choice] = &[
        Choice::new("orange", "Orange"),
        Choice::new("red", "Red"),
        Choice::new("yellow", "Yellow"),
        Choice::new("green", "Green"),
        Choice::new("teal", "Teal"),
        Choice::new("blue", "Blue"),
        Choice::new("purple", "Purple"),
        Choice::new("pink", "Pink"),
        Choice::new("gray", "Gray"),
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
        "Interface language",
        Control::Dropdown(InterfaceLanguage::CHOICES),
        choice!(prefs.language),
    )
    .info("The language of menus, buttons and messages. More languages are on the way.")
    .keywords(&["locale", "translation", "english"]),
    SettingDef::new(
        "theme",
        APPEARANCE,
        "Theme",
        Control::Segmented(ThemeMode::CHOICES),
        choice!(prefs.theme),
    )
    .info("Dark or light windows, or whichever the system uses right now.")
    .keywords(&["appearance", "dark mode", "light mode", "color scheme"]),
    SettingDef::new(
        "accent",
        APPEARANCE,
        "Accent color",
        Control::Swatches { choices: Accent::CHOICES, color: accent_color },
        choice!(prefs.accent),
    )
    .info("The color of selected items, switches, sliders and the progress bar.")
    .keywords(&["highlight", "colour", "tint"]),
    SettingDef::new(
        "startup.show_dialogs",
        STARTUP,
        "Show confirmation and error dialogs",
        Control::Toggle,
        toggle!(prefs.startup.show_dialogs),
    )
    .info(
        "Asks before discarding or deleting something. Off, it happens at once; a deleted \
         playlist can still be brought back with Undo.",
    )
    .keywords(&["confirm", "ask", "prompt", "warning"]),
    SettingDef::new(
        "startup.check_for_updates",
        STARTUP,
        "Check for updates automatically",
        Control::Toggle,
        toggle!(prefs.startup.check_for_updates),
    )
    .info("Looks for a new version of VLC now and then, and offers to install it.")
    .keywords(&["update", "upgrade", "version", "new version"]),
    SettingDef::new(
        "run_setup_again",
        STARTUP,
        "Setup",
        Control::Button { label: "Run Setup Again…", action: run_setup_again },
        Binding::ACTION,
    )
    .info(
        "Walks through the first-run setup again: media folder, theme and shortcuts, starting \
         from your current choices. Cancel leaves everything as it was.",
    )
    .keywords(&["onboarding", "welcome", "first run", "wizard"]),
];
