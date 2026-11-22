//! A Settings page drawn from the registry (S5.1.2): the rows of one [`Category`], under their
//! section headings, with the advanced ones behind an Advanced Options bar.
//!
//! - A row whose dependency isn't met is dimmed and disabled (S5.1.3), down a chain of parents.
//! - Each `(i)` shows the row's info text (S5.1.4).
//! - Changing a `NextMedia` row adds "Applies to the next file you open" under it; while any
//!   `Restart` setting differs from what this launch started with, a "Restart VLC to apply"
//!   banner with **Restart now** tops the page (S5.1.5).
//!
//! - While the Settings window searches (S5.2.3), only the matching rows show, with the words
//!   marked, and Advanced Options opens when a match is inside it.
//! - Advanced Options stays open or closed per page for the session (S5.2.4).
//!
//! Changes apply at once: there is no Save button. The page follows the settings as they change
//! anywhere, so a value changed elsewhere shows here too. The Settings window shows a page per
//! category.

mod advanced;
mod color_picker;
mod controls;
mod equalizer;
mod highlight;
mod media_folders;
mod options;
mod row;
mod secret;
mod swatches;
#[cfg(test)]
mod tests;
mod widgets;

use std::collections::HashSet;

use gpui::{
    AnyElement, Context, IntoElement, Render, SharedString, Subscription, Window, div, prelude::*,
    px,
};

use self::controls::RowState;
use self::row::{PAGE_PADDING, RowLabel};
use self::widgets::Widgets;
use crate::app::audio_devices::AudioDevices;
use crate::settings::registry::{self, ApplyMode, Category, SettingDef, Value};
use crate::settings::{self, Settings, SettingsStore};
use crate::theme::{self, Palette};
use crate::ui::toast::{Level, banner};

/// The note under a `NextMedia` row once it was changed.
const NEXT_MEDIA_NOTE: &str = "Applies to the next file you open";

/// One category's page.
pub struct SettingsPage {
    category: Category,
    /// The search the page answers (folded words); empty shows every row.
    words: Vec<String>,
    /// Advanced Options opened or closed by hand during the current search.
    search_advanced: Option<bool>,
    widgets: Widgets,
    /// `NextMedia` rows changed on this page, which say when the change applies.
    edited: HashSet<&'static str>,
    _settings: Subscription,
    _devices: Subscription,
}

impl SettingsPage {
    /// The page of `category`, with its controls showing the current settings.
    pub fn new(category: Category, window: &mut Window, cx: &mut Context<Self>) -> Self {
        theme::observe(window, cx);
        let settings = cx.observe_global_in::<SettingsStore>(window, |this, window, cx| {
            this.sync_widgets(window, cx);
            cx.notify();
        });
        // Options rows list what the machine has; the audio devices change while Settings shows.
        let devices = cx.observe_global_in::<AudioDevices>(window, |this, window, cx| {
            this.sync_widgets(window, cx);
        });
        let defs: Vec<_> = registry::in_category(category).collect();
        let widgets = Widgets::new(&defs, window, cx);
        let page = Self {
            category,
            words: Vec::new(),
            search_advanced: None,
            widgets,
            edited: HashSet::new(),
            _settings: settings,
            _devices: devices,
        };
        page.widgets.sync(&defs, window, cx);
        page
    }

    /// Shows the saved values in the stateful controls.
    fn sync_widgets(&self, window: &mut Window, cx: &mut Context<Self>) {
        let defs: Vec<_> = registry::in_category(self.category).collect();
        self.widgets.sync(&defs, window, cx);
    }

    /// Shows only the rows matching `words` (from [`registry::search_words`]); none shows all.
    pub fn set_search(&mut self, words: Vec<String>, cx: &mut Context<Self>) {
        if self.words != words {
            self.words = words;
            self.search_advanced = None;
            cx.notify();
        }
    }

    /// Opens or closes the page's Advanced Options. During a search this lasts until the search
    /// changes; otherwise it is remembered for the session.
    pub fn set_advanced_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if self.words.is_empty() {
            advanced::set_open(self.category, open, cx);
        } else {
            self.search_advanced = Some(open);
        }
        cx.notify();
    }

    /// Whether Advanced Options shows its rows: opened by hand, or holding a search match.
    fn is_advanced_open(&self, has_advanced_rows: bool, cx: &Context<Self>) -> bool {
        if self.words.is_empty() {
            advanced::is_open(self.category, cx)
        } else {
            self.search_advanced.unwrap_or(has_advanced_rows)
        }
    }

    /// The rows the page shows: all of them, or those the search finds.
    fn shown_rows(&self) -> impl Iterator<Item = &'static SettingDef> + '_ {
        registry::in_category(self.category)
            .filter(|def| self.words.is_empty() || def.matches(&self.words))
    }

    /// Saves `value` for `def` if the setting takes it.
    fn write(&mut self, def: &'static SettingDef, value: Value, cx: &mut Context<Self>) {
        let Ok(value) = def.accept(value) else { return };
        if !def.is_enabled(settings::get(cx)) || def.value(settings::get(cx)) == value {
            return;
        }
        settings::update(cx, |settings| (def.binding.set)(settings, value));
        if def.apply == ApplyMode::NextMedia {
            self.edited.insert(def.key);
        }
        cx.notify();
    }

    /// The rows of `defs`, under a heading per section in the order the sections first appear.
    fn sections(
        &self,
        defs: &[&'static SettingDef],
        settings: &Settings,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut titles: Vec<&'static str> = Vec::new();
        for def in defs {
            if !titles.contains(&def.section) {
                titles.push(def.section);
            }
        }
        let mut elements = Vec::new();
        for title in titles {
            elements.push(row::section_title(title, p).into_any_element());
            for def in defs.iter().filter(|def| def.section == title) {
                elements.push(self.row(def, settings, p, cx).into_any_element());
            }
        }
        elements
    }

    fn row(
        &self,
        def: &'static SettingDef,
        settings: &Settings,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let state = RowState { value: def.value(settings), enabled: def.is_enabled(settings) };
        let hint = def.hint.and_then(|hint| hint(cx)).map(SharedString::from);
        let edited = self.edited.contains(def.key).then_some(SharedString::from(NEXT_MEDIA_NOTE));
        let note = hint.or(edited);
        let label = RowLabel {
            key: def.key,
            label: def.label,
            highlights: highlight::ranges(def.label, &self.words),
            info: def.info,
            note,
            enabled: state.enabled,
        };
        let control = self.control(def, &state, p, cx);
        if matches!(def.control, registry::Control::Equalizer | registry::Control::MediaFolders) {
            // The panel is the row: the design gives the equalizer the page's width and no
            // label, and the folder list, under its own heading, follows it. Dimmed like any
            // row while its dependency isn't met.
            return div()
                .py(px(8.))
                .when(!state.enabled, |panel| panel.opacity(row::DISABLED_OPACITY))
                .child(control);
        }
        row::row(&label, control, p)
    }

    fn write_choice(&mut self, def: &'static SettingDef, index: usize, cx: &mut Context<Self>) {
        if let Some(choice) = def.choices().get(index) {
            self.write(def, Value::Choice(choice.id), cx);
        }
    }

    /// "Restart VLC to apply", while a restart setting was changed.
    fn restart_banner(cx: &Context<Self>) -> Option<AnyElement> {
        settings::needs_restart(cx).then(|| {
            banner("restart-banner", Level::Warning, "Restart VLC to apply")
                .action("Restart now", |_, cx| settings::restart(cx))
                .into_any_element()
        })
    }
}

impl Render for SettingsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let settings = settings::get(cx).clone();
        let (advanced, basic): (Vec<_>, Vec<_>) = self.shown_rows().partition(|def| def.advanced);
        let basic_rows = self.sections(&basic, &settings, &p, cx);
        let open = self.is_advanced_open(!advanced.is_empty(), cx);
        let advanced_rows =
            if open { self.sections(&advanced, &settings, &p, cx) } else { Vec::new() };
        let empty_note = if self.words.is_empty() {
            "Nothing to set here yet."
        } else {
            "No settings on this page match the search."
        };
        let advanced_bar = (!advanced.is_empty()).then(|| {
            row::advanced_bar(open, &p).on_click(cx.listener(move |this, _, _, cx| {
                this.set_advanced_open(!open, cx);
            }))
        });
        div()
            .id("settings-page")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .px(px(PAGE_PADDING))
            .pb(px(PAGE_PADDING))
            .children(
                Self::restart_banner(cx).map(|banner| div().pt(px(PAGE_PADDING)).child(banner)),
            )
            .when(basic.is_empty() && advanced.is_empty(), |page| {
                page.child(row::empty_note(empty_note, &p))
            })
            .children(basic_rows)
            .children(advanced_bar)
            .children(advanced_rows)
    }
}
