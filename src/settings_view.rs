//! A Settings page drawn from the registry (S5.1.2): the rows of one [`Category`], under their
//! section headings, with the advanced ones behind an Advanced Options bar.
//!
//! - A row whose dependency isn't met is dimmed and disabled (S5.1.3), down a chain of parents.
//! - Each `(i)` shows the row's info text (S5.1.4).
//! - Changing a `NextMedia` row adds "Applies to the next file you open" under it; while any
//!   `Restart` setting differs from what this launch started with, a "Restart VLC to apply"
//!   banner with **Restart now** tops the page (S5.1.5).
//!
//! Changes apply at once: there is no Save button. The page follows the settings as they change
//! anywhere, so a value changed elsewhere shows here too. The Settings window (S5.2) shows pages,
//! except for categories whose [`Layout`] is hand-built.
//!
//! [`Layout`]: crate::settings::registry::Layout

mod controls;
mod row;
mod swatches;
#[cfg(test)]
mod tests;
mod widgets;

use std::collections::HashSet;

use gpui::{AnyElement, Context, IntoElement, Render, Subscription, Window, div, prelude::*, px};

use self::controls::RowState;
use self::row::{PAGE_PADDING, RowLabel};
use self::widgets::Widgets;
use crate::settings::registry::{self, ApplyMode, Category, SettingDef, Value};
use crate::settings::{self, Settings, SettingsStore};
use crate::theme::{self, Palette};
use crate::ui::toast::{Level, banner};

/// The note under a `NextMedia` row once it was changed.
const NEXT_MEDIA_NOTE: &str = "Applies to the next file you open";

/// One category's page.
pub struct SettingsPage {
    category: Category,
    advanced_open: bool,
    widgets: Widgets,
    /// `NextMedia` rows changed on this page, which say when the change applies.
    edited: HashSet<&'static str>,
    _settings: Subscription,
}

impl SettingsPage {
    /// The page of `category`, with its controls showing the current settings.
    pub fn new(category: Category, window: &mut Window, cx: &mut Context<Self>) -> Self {
        theme::observe(window, cx);
        let settings = cx.observe_global_in::<SettingsStore>(window, |this, window, cx| {
            this.sync_widgets(window, cx);
            cx.notify();
        });
        let defs: Vec<_> = registry::in_category(category).collect();
        let widgets = Widgets::new(&defs, window, cx);
        let page = Self {
            category,
            advanced_open: false,
            widgets,
            edited: HashSet::new(),
            _settings: settings,
        };
        page.widgets.sync(&defs, window, cx);
        page
    }

    /// Shows the saved values in the stateful controls.
    fn sync_widgets(&self, window: &mut Window, cx: &mut Context<Self>) {
        let defs: Vec<_> = registry::in_category(self.category).collect();
        self.widgets.sync(&defs, window, cx);
    }

    /// Opens or closes the page's Advanced Options.
    pub fn set_advanced_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.advanced_open = open;
        cx.notify();
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
        let note = self.edited.contains(def.key).then_some(NEXT_MEDIA_NOTE);
        let label = RowLabel {
            key: def.key,
            label: def.label,
            info: def.info,
            note,
            enabled: state.enabled,
        };
        let control = self.control(def, &state, p, cx);
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
        let (advanced, basic): (Vec<_>, Vec<_>) =
            registry::in_category(self.category).partition(|def| def.advanced);
        let basic_rows = self.sections(&basic, &settings, &p, cx);
        let advanced_rows = if self.advanced_open {
            self.sections(&advanced, &settings, &p, cx)
        } else {
            Vec::new()
        };
        let open = self.advanced_open;
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
            .when(basic.is_empty() && advanced.is_empty(), |page| page.child(row::empty_note(&p)))
            .children(basic_rows)
            .children(advanced_bar)
            .children(advanced_rows)
    }
}
